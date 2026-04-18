use std::sync::Arc;

use crate::{config::Config, dal::Store};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn Store>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: Arc<dyn Store>, config: Config) -> Self {
        Self {
            db,
            config: Arc::new(config),
        }
    }
}
