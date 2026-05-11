use tokio::sync::broadcast;

#[derive(Clone, Debug, serde::Serialize)]
pub struct StateChange {
    pub session_id: String,
    pub status: String,
    pub label: Option<String>,
    pub reason: Option<String>,
}

pub struct Bus { tx: broadcast::Sender<StateChange> }

impl Bus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(128);
        Self { tx }
    }
    pub fn subscribe(&self) -> broadcast::Receiver<StateChange> { self.tx.subscribe() }
    pub fn emit(&self, c: StateChange) { let _ = self.tx.send(c); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn emit_then_receive() {
        let bus = Bus::new();
        let mut rx = bus.subscribe();
        bus.emit(StateChange {
            session_id: "s".into(), status: "needs_input".into(),
            label: None, reason: None,
        });
        let c = rx.recv().await.unwrap();
        assert_eq!(c.status, "needs_input");
    }
}
