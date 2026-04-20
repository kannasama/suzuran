use std::sync::Arc;

use webauthn_rs::Webauthn;

use crate::{
    config::Config,
    dal::Store,
    services::{freedb::FreedBService, musicbrainz::MusicBrainzService},
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn Store>,
    pub config: Arc<Config>,
    pub webauthn: Arc<Webauthn>,
    pub mb_service: Arc<MusicBrainzService>,
    pub freedb_service: Arc<FreedBService>,
}

impl AppState {
    pub fn new(
        db: Arc<dyn Store>,
        config: Config,
        webauthn: Webauthn,
        mb_service: Arc<MusicBrainzService>,
        freedb_service: Arc<FreedBService>,
    ) -> Self {
        Self {
            db,
            config: Arc::new(config),
            webauthn: Arc::new(webauthn),
            mb_service,
            freedb_service,
        }
    }
}
