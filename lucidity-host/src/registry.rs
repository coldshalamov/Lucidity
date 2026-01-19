use std::sync::{Arc, Mutex};
use dashmap::DashMap;
use tokio::sync::mpsc;
use lucidity_proto::protocol::JsonResponse;
use once_cell::sync::Lazy;
use log::debug;

pub type ClientId = String;

pub static REGISTRY: Lazy<ClientRegistry> = Lazy::new(|| ClientRegistry::new());

pub struct ClientRegistry {
    clients: DashMap<ClientId, mpsc::UnboundedSender<JsonResponse>>,
}

impl ClientRegistry {
    pub fn new() -> Self {
        Self {
            clients: DashMap::new(),
        }
    }

    pub fn register(&self, id: ClientId, tx: mpsc::UnboundedSender<JsonResponse>) {
        debug!("Registering client {} for push notifications", id);
        self.clients.insert(id, tx);
    }

    pub fn unregister(&self, id: &ClientId) {
        debug!("Unregistering client {}", id);
        self.clients.remove(id);
    }

    pub fn broadcast(&self, msg: JsonResponse) {
        for mut entry in self.clients.iter_mut() {
            let tx = entry.value_mut();
            if let Err(_) = tx.send(msg.clone()) {
                // Client probably disconnected, but we let unregister handle it 
                // or we could remove here too.
            }
        }
    }
}
