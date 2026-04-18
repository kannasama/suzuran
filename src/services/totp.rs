use totp_rs::{Algorithm, Secret, TOTP};

use crate::error::AppError;

const TOTP_ISSUER: &str = "suzuran";
const TOTP_DIGITS: usize = 6;
const TOTP_STEP: u64 = 30;

pub struct TotpService;

impl TotpService {
    /// Generate a new base32 TOTP secret.
    pub fn generate_secret() -> String {
        Secret::generate_secret().to_encoded().to_string()
    }

    /// Build an `otpauth://` URI for QR code generation in the UI.
    pub fn otpauth_uri(secret_b32: &str, username: &str) -> anyhow::Result<String> {
        let totp = Self::build_totp(secret_b32, username)?;
        Ok(totp.get_url())
    }

    /// Verify a 6-digit TOTP code against the stored secret.
    /// Accepts current and ±1 step to account for clock skew.
    pub fn verify(secret_b32: &str, username: &str, code: &str) -> Result<(), AppError> {
        let totp = Self::build_totp(secret_b32, username)
            .map_err(AppError::Internal)?;
        if totp.check_current(code).unwrap_or(false) {
            Ok(())
        } else {
            Err(AppError::Unauthorized)
        }
    }

    fn build_totp(secret_b32: &str, username: &str) -> anyhow::Result<TOTP> {
        let secret = Secret::Encoded(secret_b32.to_string())
            .to_bytes()
            .map_err(|e| anyhow::anyhow!("invalid TOTP secret: {e:?}"))?;

        TOTP::new(
            Algorithm::SHA1,
            TOTP_DIGITS,
            1,
            TOTP_STEP,
            secret,
            Some(TOTP_ISSUER.to_string()),
            username.to_string(),
        )
        .map_err(|e| anyhow::anyhow!("TOTP build error: {e}"))
    }
}
