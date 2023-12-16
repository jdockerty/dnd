use anyhow::Result;
use clap::Parser;
use std::{sync::Arc, time::Duration};
use tokio::net::UdpSocket;
use serde::{Serialize, Deserialize};

#[derive(Parser, Clone)]
#[command(author, version, about, long_about = None)]
struct Server {
    #[arg(short, long)]
    seed_peers: Option<Vec<String>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct State {
    /// The address of this server.
    addr: String,
    /// Known peer addresses.
    peers: Arc<Vec<String>>,
}

const HELLO: &str = "HELLO";
const GOSSIP_PORT: i32 = 5000;

async fn startup_broadcast(socket: Arc<UdpSocket>) -> Result<()> {
    let broadcast = format!("{}:{}", std::net::Ipv4Addr::BROADCAST, GOSSIP_PORT);

    match socket.send_to(HELLO.as_bytes(), broadcast).await {
        Ok(n) => {
            if n != HELLO.len() {
                println!("did not write correct bytes");
            }
        }
        Err(e) => println!("Unable to write: {e}"),
    };

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting!");
    let server = Server::parse();
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    let addr = socket.local_addr()?;
    let bound_addr = format!("0.0.0.0:{}", addr);
    socket.set_broadcast(true)?;
    println!("Bound to {}", addr);

    let state = State {
        addr: bound_addr,
        peers: Vec::new().into()
        
    };

    let recv_socket = socket.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3));
        println!("Starting recv");
        loop {
            interval.tick().await;
            println!("Rec tick");
            recv_socket.readable().await.unwrap();
            match recv_socket.try_recv(&mut Vec::new()) {
                Ok(n) => {
                    println!("{n}")
                },
                    Err(e) => println!("Was not readable: {e}")

            }
        }
    })
    .await?;

    tokio::spawn(async move {
        println!("Starting send");
        let send_socket = socket.clone();
        let mut interval = tokio::time::interval(Duration::from_secs(3));
        loop {
            interval.tick().await;
            println!("Sending tick");
            match &server.seed_peers {
                Some(peers) => {
                    println!("Iterating peers");
                    for peer in peers {
                        match send_socket.send_to(HELLO.as_bytes(), peer).await {
                            Ok(n) => {
                                println!("Wrote {n}");
                            }
                            Err(e) => println!("Unable to write: {e}"),
                        }
                    }
                }
                None => {
                    println!("Broadcasting");
                    startup_broadcast(send_socket.clone()).await.unwrap();
                }
            }
        }
    })
    .await?;

    let sigint_handle = tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(_) => println!("Cancelled"),
            Err(e) => println!("error on cancel: {e}"),
        }
    })
    .await;
    sigint_handle.unwrap();

    Ok(())
}
