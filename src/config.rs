use anyhow::Context;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub port: u16,
    pub log_level: String,
    /// WebAuthn Relying Party ID — usually the domain, e.g. "localhost"
    pub rp_id: String,
    /// WebAuthn Relying Party Origin — e.g. "http://localhost:3000"
    pub rp_origin: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .context("DATABASE_URL is required")?,
            jwt_secret: std::env::var("JWT_SECRET")
                .context("JWT_SECRET is required")?,
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".into())
                .parse()
                .context("PORT must be a valid port number")?,
            log_level: std::env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".into()),
            rp_id: std::env::var("RP_ID")
                .unwrap_or_else(|_| "localhost".into()),
            rp_origin: std::env::var("RP_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:3000".into()),
        })
    }
}
