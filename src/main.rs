#![feature(async_closure)]

use std::time::{Duration, Instant};

use async_stream::stream;
use bytes::Bytes;
use clap::{App, Arg};
use failure_derive::Fail;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures::Stream;
use serde_json::json;
use warp::http::header::{HeaderMap, HeaderValue};
use warp::ws::{WebSocket, Ws};
use warp::Filter;

type ResultFailure<O> = Result<O, failure::Error>;

const INDEX_HTML_BYTES: &[u8] = include_bytes!("../static/index.html");
const JITTER_HTML_BYTES: &[u8] = include_bytes!("../static/jitter.html");
const LOOP_HTML_BYTES: &[u8] = include_bytes!("../static/loop.html");
const LISTEN_ADDR: &str = "LISTEN_ADDR";

#[tokio::main]
async fn main() -> ResultFailure<()> {
    let matches = App::new("pingtest")
        .arg(
            Arg::with_name(LISTEN_ADDR)
                .default_value("[::]:8080")
                .long("listen")
                .required(true)
                .short("l")
                .takes_value(true),
        )
        .get_matches();

    let listen_addr = matches
        .value_of(LISTEN_ADDR)
        .unwrap()
        .parse::<std::net::SocketAddr>()?;

    let ws_route = warp::path("ws")
        .and(warp::ws())
        .map(|w: Ws| w.on_upgrade(handle_ws_void));

    let ws2_route = warp::path("ws2")
        .and(warp::ws())
        .map(|w: Ws| w.on_upgrade(handle_ws2_void));

    let jitter_sse_route = warp::path("jitter.sse")
        .and(warp::get())
        .map(|| warp::sse::reply(warp::sse::keep_alive().stream(jitter_sse_events())));

    let echo_route = warp::path("echo")
        .and(warp::post())
        .and(warp::filters::body::content_length_limit(64 * 1024 - 1))
        .and(warp::filters::body::bytes())
        .map(handle_echo);

    let html_page = |b| {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Cross-Origin-Embedder-Policy",
            HeaderValue::from_static("require-corp"),
        );
        headers.insert(
            "Cross-Origin-Opener-Policy",
            HeaderValue::from_static("same-origin"),
        );
        warp::any()
            .map(move || warp::http::Response::builder().body(b))
            .with(warp::reply::with::headers(headers))
    };

    let jitter_html_route = warp::path("jitter.html").and(html_page(JITTER_HTML_BYTES));
    let loop_html_route = warp::path("loop.html").and(html_page(LOOP_HTML_BYTES));
    let index_html_route = warp::path::end().and(html_page(INDEX_HTML_BYTES));

    let routes = ws_route
        .or(echo_route)
        .or(jitter_html_route)
        .or(jitter_sse_route)
        .or(loop_html_route)
        .or(ws2_route)
        .or(index_html_route);

    warp::serve(routes).run(listen_addr).await;

    Ok(())
}

fn handle_echo(b: Bytes) -> Result<warp::http::Response<Bytes>, warp::http::Error> {
    warp::http::Response::builder()
        .status(200)
        .header("Content-Type", "application/octet-stream")
        .body(b)
}

async fn handle_ws_void(sock: WebSocket) -> () {
    let _ = handle_ws(sock).await;
}

#[derive(Fail, Debug)]
#[fail(display = "EndOfStreamError")]
struct EndOfStreamError {}

async fn handle_ws(mut sock: WebSocket) -> ResultFailure<()> {
    loop {
        let msg = sock.next().await.ok_or(EndOfStreamError {})??;
        sock.send(msg).await?;
    }
}

async fn handle_ws2_void(sock: WebSocket) -> () {
    let _ = handle_ws2(sock).await;
}

async fn handle_ws2(mut sock: WebSocket) -> ResultFailure<()> {
    let first_msg = sock.next().await.ok_or(EndOfStreamError {})??;
    if b"\x01" == first_msg.as_bytes() {
        // jitter 50 ms
        let start_instant = Instant::now();
        loop {
            let instant = Instant::now();
            let ts = (instant - start_instant).as_nanos() as f64 / 1_000_000.; // millis
            let msg = warp::ws::Message::binary(ts.to_le_bytes());
            sock.send(msg).await?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    } else {
        Ok(())
    }
}

fn jitter_sse_events() -> impl Stream<Item = Result<warp::sse::Event, std::convert::Infallible>> {
    stream! {
        let start_instant = Instant::now();
        loop {
            let instant = Instant::now();
            let ts = (instant - start_instant).as_nanos() as f64 / 1_000_000.; // millis
            let msg = json!(ts).to_string();
            yield Ok(warp::sse::Event::default().data(msg));
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
