use anyhow::Result;
use clap::Parser;
use gossip::server::{Peer, Server};
use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Parser, Debug)]
struct Kv {
    #[arg(long, short, default_value = "5000")]
    port: String,

    #[arg(long, short, default_value = "127.0.0.1")]
    address: String,

    #[arg(long, short)]
    join: Option<Peer>,

    #[arg(long, short)]
    start: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let kv = Kv::parse();
    let addr = format!("{}:{}", kv.address, kv.port);
    let socket = Arc::new(UdpSocket::bind(addr).await.unwrap());
    let server = Server::new(socket.clone())?;
    println!("Running on {}", socket.clone().local_addr().unwrap());

    if kv.start && kv.join.is_some() {
        println!("--join and --start are mutually exclusive");
    };

    if kv.start {
        server.run(gossip::server::Operation::Start).await?;
    }

    server
        .run(gossip::server::Operation::Join(kv.join.unwrap()))
        .await?;

    Ok(())
}
