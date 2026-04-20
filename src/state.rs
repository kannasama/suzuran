use std::sync::Arc;

use webauthn_rs::Webauthn;

use crate::{config::Config, dal::Store, services::musicbrainz::MusicBrainzService};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn Store>,
    pub config: Arc<Config>,
    pub webauthn: Arc<Webauthn>,
    pub mb_service: Arc<MusicBrainzService>,
}

impl AppState {
    pub fn new(
        db: Arc<dyn Store>,
        config: Config,
        webauthn: Webauthn,
        mb_service: Arc<MusicBrainzService>,
    ) -> Self {
        Self {
            db,
            config: Arc::new(config),
            webauthn: Arc::new(webauthn),
            mb_service,
        }
    }
}
