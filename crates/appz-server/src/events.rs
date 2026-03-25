use tokio::sync::broadcast;

use crate::protocol::ServerEvent;

pub struct EventBus {
    sender: broadcast::Sender<ServerEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: ServerEvent) {
        // Ignore send errors (no active subscribers is fine)
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }
}
