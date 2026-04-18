use std::sync::Arc;

use webauthn_rs::Webauthn;

use crate::{config::Config, dal::Store};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn Store>,
    pub config: Arc<Config>,
    pub webauthn: Arc<Webauthn>,
}

impl AppState {
    pub fn new(db: Arc<dyn Store>, config: Config, webauthn: Webauthn) -> Self {
        Self {
            db,
            config: Arc::new(config),
            webauthn: Arc::new(webauthn),
        }
    }
}
