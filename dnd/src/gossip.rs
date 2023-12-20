use anyhow::{anyhow, Result};
use chrono::prelude::*;
use dashmap::mapref::entry::Entry::{Occupied, Vacant};
use dashmap::DashMap;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::{net::UdpSocket, sync::RwLock};

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    peers: Vec<Peer>,
    update: Update,
}

#[derive(Debug, Serialize, Deserialize)]
struct Update {
    timestamp: chrono::DateTime<chrono::Utc>,
    store: Arc<DashMap<String, serde_json::Value>>,
}

#[derive(Clone)]
pub struct Server {
    local: Peer,
    peers: Arc<RwLock<Vec<Peer>>>,
    socket: Arc<UdpSocket>,
    pub store: Arc<DashMap<String, serde_json::Value>>,
    // liveness: Arc<RwLock<HashMap<Peer, tokio::time::Instant>>>,
}

impl Server {
    pub fn new(socket: Arc<UdpSocket>) -> Result<Self> {
        let local = Peer {
            address: socket.local_addr()?,
        };

        Ok(Self {
            local: local.clone(),
            peers: Arc::new(RwLock::new(vec![local.clone()])),
            socket,
            store: Arc::new(DashMap::new()),
            // liveness: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn write(&self) -> Result<()> {
        self.socket.writable().await?;
        let mut rng = thread_rng();
        let peers = self.peers.read().await;

        let i = rng.gen_range(0..peers.len());

        let update = Update {
            timestamp: Utc::now(),
            store: self.store.clone(),
        };

        let message = serde_json::to_vec(&Message {
            peers: peers.to_vec(),
            update,
        })?;

        let chosen = peers[i].clone();

        if chosen == self.local {
            println!("Self chosen, skipping this round");
            return Ok(());
        }

        drop(peers);
        self.socket.send_to(&message, chosen.address).await?;
        Ok(())
    }

    async fn read(&self) -> Result<()> {
        self.socket.readable().await?;
        let mut buf = Vec::new();
        buf.resize(4096, 0);
        let (n, _addr) = self.socket.recv_from(&mut buf).await?;
        buf.truncate(n);

        if n == 0 {
            return Err(anyhow!("Empty data"));
        }

        let incoming: Message = serde_json::from_slice(&buf[..n])?;

        for peer in &incoming.peers {
            if self.peers.read().await.contains(peer) {
                println!("Exists, skipping");
                for k in incoming.update.store.iter() {
                    match self.store.entry(k.to_string()) {
                        Vacant(_) => {
                            self.store.insert(k.key().to_string(), k.value().clone());
                        }
                        Occupied(_) => {
                            // TODO: liveness for comparing timestamps
                            self.store.insert(k.key().to_string(), k.value().clone());
                        }
                    }
                }
                continue;
            } else {
                if *peer == self.local {
                    println!("Not adding self");
                    continue;
                }

                let mut write = self.peers.write().await;
                write.push(peer.clone());
                for k in incoming.update.store.iter() {
                    self.store
                        .entry(k.key().to_string())
                        .or_insert(k.value().clone());
                }
            }
        }

        Ok(())
    }

    async fn event_loop(&self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            interval.tick().await;
            tokio::select! {
                _ = self.read() => {
                println!("KV {:?}", self.store);
                }
                _ = self.write() => {
                println!("KV {:?}", self.store);
                }
            }
        }
    }

    pub async fn start(&self) -> Result<()> {
        println!("Starting new cluster");
        self.event_loop().await?;
        Ok(())
    }

    pub async fn join(&self, peer: Peer) -> Result<()> {
        println!("Joining cluster using {} as known peer", peer.address);

        // UDP is unreliable, so send it a few times.
        // We could use TCP here for reliable delivery of initial sync
        // (like serf), but I'm not interested in getting fully into the
        // weeds of gossip for this project.
        for _ in 0..=3 {
            self.socket.writable().await?;
            let msg = serde_json::to_vec(&Message {
                peers: vec![self.local.clone()],
                update: Update {
                    // This is a hack to make sure we don't overwrite the
                    // store with an empty one.
                    timestamp: Utc::now() - Duration::from_secs(6000),
                    store: self.store.clone(),
                },
            })?;
            match self.socket.try_send_to(&msg, peer.address) {
                Ok(_) => println!("Sent to {}", peer.address),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    println!("Would block");
                }
                Err(e) => {
                    println!("Error with {}: {e}", peer.address);
                }
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        self.event_loop().await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct Peer {
    pub address: SocketAddr,
}

impl FromStr for Peer {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse the string into a SocketAddr
        match s.parse() {
            Ok(address) => Ok(Peer { address }),
            Err(err) => Err(format!("Failed to parse peer: {}", err)),
        }
    }
}
