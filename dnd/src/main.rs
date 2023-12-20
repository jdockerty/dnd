use anyhow::Result;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::Response;
use axum::{
    body::Bytes,
    routing::{get, post},
    Router,
};
use clap::{Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use dashmap::mapref::entry::Entry::{Occupied, Vacant};
use gossip::Kv;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::info;

mod gossip;

#[derive(Parser, Debug)]
struct App {
    #[arg(long, short, default_value = "5000")]
    port: String,

    #[arg(long, default_value = "6000")]
    server_port: String,

    #[arg(long, short, default_value = "127.0.0.1")]
    address: String,

    #[command(flatten)]
    verbose: Verbosity<InfoLevel>,

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

async fn get_key(Path(key): Path<String>, State(state): State<Arc<Kv>>) -> Response<Body> {
    // Use the entry API to avoid locking entire map on key retrieval.
    match state.store.entry(key) {
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
    State(state): State<Arc<Kv>>,
    body: Bytes,
) -> Response<Body> {
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    state.store.insert(key, value);
    state.counter.fetch_add(1, SeqCst);
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
    let server = gossip::Server::new(socket.clone(), app.verbose)?;
    info!("Running on {}", socket.clone().local_addr().unwrap());

    let state = server.kv.clone();
    tokio::spawn(async move {
        info!("Starting HTTP API");
        let server = Router::new()
            .route("/health", get(health))
            .route("/kv/:key", get(get_key))
            .route("/kv/:key", post(put_key))
            .with_state(state.into());

        let addr = format!("{}:{}", app.address, app.server_port);
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        info!("Listening for HTTP on {}", listener.local_addr().unwrap());
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
