use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tokio::net::UdpSocket;

mod gossip;

#[derive(Parser, Debug)]
struct App {
    #[arg(long, short, default_value = "5000")]
    port: String,

    #[arg(long, short, default_value = "127.0.0.1")]
    address: String,

    #[arg(long, short)]
    join: Option<gossip::Peer>,

    #[arg(long, short)]
    start: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = App::parse();
    let addr = format!("{}:{}", app.address, app.port);
    let socket = Arc::new(UdpSocket::bind(addr).await.unwrap());
    let server = gossip::Server::new(socket.clone())?;
    println!("Running on {}", socket.clone().local_addr().unwrap());

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
