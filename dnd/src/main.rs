use anyhow::Result;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response, StatusCode};
use axum::{
    body::Bytes,
    routing::{get, post},
    Router,
};
use clap::{Parser, Subcommand};
use dashmap::mapref::entry::Entry::{Occupied, Vacant};
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

    #[command(subcommand)]
    cmds: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start a new cluster.
    Start,

    /// Join an existing cluster.
    Join {
        /// A known peer within an existing cluster, in the format <host>:<port>.
        peer: gossip::Peer,
    },
}

async fn get_key(
    Path(key): Path<String>,
    State(mapping): State<Arc<DashMap<String, serde_json::Value>>>,
) -> Response<Body> {
    // Use the entry API to avoid locking entire map on key retrieval.
    match mapping.entry(key) {
        Vacant(_) => Response::builder()
            .status(404)
            .body(Body::from("None"))
            .unwrap(),
        Occupied(o) => Response::builder()
            .status(200)
            .body(o.get().to_string().into())
            .unwrap(),
    }
}

async fn put_key(
    Path(key): Path<String>,
    State(mapping): State<Arc<DashMap<String, serde_json::Value>>>,
    body: Bytes,
) -> Response<Body> {
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    mapping.insert(key, value);
    Response::new("OK".into())
}

async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = App::parse();
    let addr = format!("{}:{}", app.address, app.port);
    let socket = Arc::new(UdpSocket::bind(addr).await.unwrap());
    let server = gossip::Server::new(socket.clone())?;
    println!("Running on {}", socket.clone().local_addr().unwrap());

    let mapping = server.store.clone();
    tokio::spawn(async move {
        println!("Spawning HTTP API");
        let server = Router::new()
            .route("/health", get(health))
            .route("/kv/:key", get(get_key))
            .route("/kv/:key", post(put_key))
            .with_state(mapping);

        let addr = format!("{}:{}", app.address, app.server_port);
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        println!("listening on {}", listener.local_addr().unwrap());
        axum::serve(listener, server).await.unwrap();
    });

    match app.cmds {
        Commands::Start => server.start().await?,
        Commands::Join { peer } => {
            server.join(peer).await?;
        }
    }

    Ok(())
}
