use anyhow::Result;
use std::{net::Ipv4Addr, str::FromStr, sync::Arc, time::Duration};
use tokio::io::AsyncReadExt;
use tokio::{net::UdpSocket, sync::RwLock};

#[derive(Clone)]
pub struct Server {
    peers: Option<Arc<RwLock<Vec<Peer>>>>,
    socket: Arc<UdpSocket>,
}

const MULTICAST_ADDR: &str = "224.0.0.1";
const GOSSIP_PORT: &str = "5000";

impl Server {
    pub fn new(seed_peers: Option<Arc<RwLock<Vec<Peer>>>>, socket: Arc<UdpSocket>) -> Self {
        Self {
            socket,
            peers: seed_peers,
        }
    }

    async fn write_peers(&self) {
        self.socket.writable().await.unwrap();
        if let Some(peers) = self.peers().await.unwrap() {
            println!("Writing to known peers");
            self.socket.send(b"PEERS...\n").await.unwrap();
        } else {
            println!("No peers");
            self.socket.writable().await.unwrap();
            self.socket.send(b"NONE\n").await.unwrap();
        }
    }

    async fn read_peers(&self) {
        let mut buf = [0; 1024];
        match self.socket.try_recv(&mut buf) {
            Ok(_r) => {
                println!("Got: {buf:?}");
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available yet, continue or sleep and try again
                println!("No data available yet");
            }
            Err(e) => println!("Unable to read: {e}"),
        }
    }

    pub async fn run(self) -> Result<()> {
        println!("Running server");
        self.socket
            .join_multicast_v4(
                Ipv4Addr::from_str(MULTICAST_ADDR).unwrap(),
                Ipv4Addr::UNSPECIFIED,
            )
            .unwrap();

        println!("Connecting to: {}:{}", MULTICAST_ADDR, GOSSIP_PORT);
        self.socket
            .connect(format!("{}:{}", MULTICAST_ADDR, GOSSIP_PORT))
            .await
            .unwrap();

        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            self.read_peers().await;
            self.write_peers().await;
        }
    }

    pub async fn peers(&self) -> Result<Option<Vec<Peer>>> {
        if let Some(peers) = &self.peers {
            let peers = peers.read().await;
            Ok(Some(peers.to_vec()))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone)]
pub struct Peer {
    address: String,
}
