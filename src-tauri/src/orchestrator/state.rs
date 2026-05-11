use std::sync::Arc;
use super::{db::Store, bus::Bus};

pub struct AppState {
    pub store: Arc<Store>,
    pub bus: Arc<Bus>,
}

impl AppState {
    pub fn new(store: Arc<Store>) -> Arc<Self> {
        Arc::new(Self { store, bus: Arc::new(Bus::new()) })
    }
}
