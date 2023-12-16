use std::collections::BTreeMap;

use tokio::net::UdpSocket;

/// Distributed key-value store.
pub struct Dnd {
    pub store: BTreeMap<String, String>,
    pub peers: Vec<UdpSocket>,
}

impl Dnd {
    pub fn new() -> Self {
        Self {
            store: BTreeMap::new(),
            peers: Vec::new(),
        }
    }
}
