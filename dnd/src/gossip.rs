use anyhow::{anyhow, Result};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use dashmap::DashMap;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::time::Duration;
use tokio::{net::UdpSocket, sync::RwLock};
use tracing::{debug, error, info};
use tracing_log::AsTrace;
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    peers: Vec<Peer>,
    update: Update,
}

#[derive(Debug, Serialize, Deserialize)]
struct Update(Kv);

#[derive(Clone)]
pub struct Server {
    local: Peer,
    peers: Arc<RwLock<Vec<Peer>>>,
    socket: Arc<UdpSocket>,
    pub kv: Kv,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Kv {
    pub counter: Arc<AtomicU64>,
    pub store: Arc<DashMap<String, serde_json::Value>>,
}

impl Server {
    pub fn new(socket: Arc<UdpSocket>, verbosity: Verbosity<InfoLevel>) -> Result<Self> {
        let _ = FmtSubscriber::builder()
            .with_max_level(verbosity.log_level_filter().as_trace())
            .try_init();

        let local = Peer {
            address: socket.local_addr()?,
        };

        let kv_store = Kv {
            counter: Arc::new(AtomicU64::new(0)),
            store: Arc::new(DashMap::new()),
        };

        Ok(Self {
            local: local.clone(),
            peers: Arc::new(RwLock::new(vec![local.clone()])),
            socket,
            kv: kv_store,
        })
    }

    async fn write(&self) -> Result<()> {
        self.socket.writable().await?;
        let mut rng = thread_rng();
        let peers = self.peers.read().await.to_vec();
        let peers: Vec<Peer> = peers
            .iter()
            .map(|p| p.to_owned())
            .filter(|p| *p != self.local)
            .collect();

        // If we're the only peer in the filtered vector, don't send anything.
        // There is no need to send messages to ourselves.
        if peers.is_empty() {
            debug!("No peers in cluster.");
            return Ok(());
        }

        let i = rng.gen_range(0..peers.len());
        let chosen = peers[i].clone();
        let update = Update(self.kv.clone());
        let message = serde_json::to_vec(&Message {
            peers: peers.clone(),
            update,
        })?;
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
        let incoming_counter = incoming.update.0.counter.load(SeqCst);

        if incoming_counter > self.kv.counter.load(SeqCst) {
            info!("Incoming update has greater counter, updating");
            self.kv.counter.swap(incoming_counter, SeqCst);
            for k in incoming.update.0.store.iter() {
                self.kv.store.insert(k.key().to_string(), k.value().clone());
            }
        }

        for peer in &incoming.peers {
            if self.peers.read().await.contains(peer) {
                continue;
            } else {
                if *peer == self.local {
                    continue;
                }
                let mut write = self.peers.write().await;
                write.push(peer.clone());
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
                info!("Store: {:?}", self.kv.store);
                }
                _ = self.write() => {
                info!("Store: {:?}", self.kv.store);
                }
            }
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting new cluster");
        self.event_loop().await?;
        Ok(())
    }

    pub async fn join(&self, peer: Peer) -> Result<()> {
        info!("Joining cluster using {} as known peer", peer.address);

        // UDP is unreliable, so send it a few times.
        // We could use TCP here for reliable delivery of initial sync
        // (like serf), but I'm not interested in getting fully into the
        // weeds of gossip for this project.
        for _ in 0..=3 {
            self.socket.writable().await?;
            let msg = serde_json::to_vec(&Message {
                peers: vec![self.local.clone()],
                update: Update(self.kv.clone()),
            })?;
            match self.socket.try_send_to(&msg, peer.address) {
                Ok(_) => debug!("Initial update sent to {}", peer.address),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    error!("Would block");
                }
                Err(e) => {
                    error!("Error with {}: {e}", peer.address);
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
