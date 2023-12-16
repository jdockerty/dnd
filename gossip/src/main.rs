use anyhow::Result;
use tokio::net::UdpSocket;
use clap::Parser;
use std::collections::BTreeMap;
use tokio::net::UdpSocket;


#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Server {

    #[arg(short, long)]
    seed_peers: Option<Vec<String>>
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting!");
    let server = Server::parse();
        let socket = UdpSocket::bind("127.0.0.1:5000").await?;
        println!("Bound to {}", socket.local_addr()?);

    // Idea: on startup take a vec of seed peers. Connect to these and send/recv
    // from them.
    // How to handle starting from 0? Receive from bound socket and other peers send their known
    // peers.
    tokio::task::spawn(async move {

        if let Some(seed_peers) = server.seed_peers {
            for peer in seed_peers {
                socket.connect(peer).await.unwrap();

            }

        }
    })
    .await;

    match tokio::signal::ctrl_c().await {
        Ok(_) => println!("Cancelled"),
        Err(e) => println!("error on cancel: {e}"),
    }

    Ok(())
}
