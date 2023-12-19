use anyhow::Result;
use axum::extract::{Path, State};
use axum::{
    body::Bytes,
    routing::{get, post},
    Router,
};
use clap::Parser;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::net::UdpSocket;

mod gossip;

#[derive(Parser, Debug)]
struct App {
    #[arg(long, short, default_value = "5000")]
    port: String,

    #[arg(long, default_value = "6000")]
    server_port: String,

    #[arg(long, short, default_value = "127.0.0.1")]
    address: String,

    #[arg(long, short)]
    join: Option<gossip::Peer>,

    #[arg(long, short)]
    start: bool,
}

async fn get_key(
    Path(key): Path<String>,
    State(mapping): State<Arc<DashMap<String, serde_json::Value>>>,
) -> Bytes {
    // Use the entry API to avoid locking entire map on key retrieval.
    match mapping.entry(key) {
        dashmap::mapref::entry::Entry::Vacant(_) => "None".into(),
        dashmap::mapref::entry::Entry::Occupied(o) => o.get().to_string().into_bytes().into(),
    }
}

async fn put_key(
    Path(key): Path<String>,
    State(mapping): State<Arc<DashMap<String, serde_json::Value>>>,
    body: Bytes,
) -> Bytes {
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    mapping.insert(key, value);
    "OK".into()
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = App::parse();
    let addr = format!("{}:{}", app.address, app.port);
    let socket = Arc::new(UdpSocket::bind(addr).await.unwrap());
    let server = gossip::Server::new(socket.clone())?;
    println!("Running on {}", socket.clone().local_addr().unwrap());

    println!("Spawning HTTP API");
    let mapping = server.store.clone();
    tokio::spawn(async move {
        let server = Router::new()
            .route("/:key", get(get_key))
            .route("/:key", post(put_key))
            .with_state(mapping);

        let addr = format!("127.0.0.1:{}", app.server_port);
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        println!("listening on {}", listener.local_addr().unwrap());
        axum::serve(listener, server).await.unwrap();
    });

    if app.start && app.join.is_some() {
        println!("--join and --start are mutually exclusive");
    };

    if app.start {
        server.run(gossip::Operation::Start).await?;
    }

    server
        .run(gossip::Operation::Join(app.join.unwrap()))
        .await?;

    Ok(())
}
