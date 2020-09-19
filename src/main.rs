use clap::{App, Arg};
use failure_derive::Fail;
use futures_util::sink::SinkExt;
use tokio::stream::StreamExt;
use warp::Filter;
use warp::http::header::{HeaderMap, HeaderValue};
use warp::ws::{WebSocket, Ws};

type ResultFailure<O> = Result<O, failure::Error>;

const LISTEN_ADDR: &str = "LISTEN_ADDR";

#[tokio::main]
async fn main() -> ResultFailure<()> {
    let matches = App::new("pingtest")
        .arg(Arg::with_name(LISTEN_ADDR)
             .default_value("[::]:8080")
             .long("listen")
             .required(true)
             .short("l")
             .takes_value(true))
        .get_matches();

    let listen_addr = matches.value_of(LISTEN_ADDR).unwrap().parse::<std::net::SocketAddr>()?;

    let ws_route = warp::path("ws").and(warp::ws()).map(|w: Ws| {
        w.on_upgrade(handle_ws_void)
    });

    let mut headers = HeaderMap::new();
    headers.insert("Cross-Origin-Embedder-Policy", HeaderValue::from_static("require-corp"));
    headers.insert("Cross-Origin-Opener-Policy", HeaderValue::from_static("same-origin"));
    let static_route = warp::path::end().and(warp::fs::dir("static")).with(warp::reply::with::headers(headers));

    let routes = ws_route.or(static_route);

    warp::serve(routes).run(listen_addr).await;

    Ok(())
}

async fn handle_ws_void(sock: WebSocket) -> () {
    let _ = handle_ws(sock).await;
}

#[derive(Fail, Debug)]
#[fail(display = "EndOfStreamError")]
struct EndOfStreamError {
}

async fn handle_ws(mut sock: WebSocket) -> ResultFailure<()> {
    loop {
        let msg = sock.next().await.ok_or(EndOfStreamError{})??;
        sock.send(msg).await?;
    }
}
