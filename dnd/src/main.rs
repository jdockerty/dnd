use anyhow::Result;
use gossip::server::Server;
use std::sync::Arc;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> Result<()> {
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap());
    let server = Server::new(None, socket.clone());
    println!("Running on {}", socket.clone().local_addr().unwrap());

    server.run().await?;

    Ok(())
}
