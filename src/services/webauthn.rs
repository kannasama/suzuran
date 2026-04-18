use std::sync::Arc;

use webauthn_rs::prelude::*;

use crate::{dal::Store, error::AppError, models::WebauthnCredential};

pub struct WebauthnService;

impl WebauthnService {
    /// Start passkey registration for a user. Returns JSON challenge to send to browser.
    /// Persists challenge state to DB.
    pub async fn start_registration(
        webauthn: &Webauthn,
        db: &Arc<dyn Store>,
        user_id: i64,
        username: &str,
    ) -> Result<serde_json::Value, AppError> {
        let user_unique_id = uuid::Uuid::from_u64_pair(0, user_id as u64);

        // Reconstruct CredentialIDs from stored Passkey JSON to build the exclude list.
        let exclude_credentials: Vec<CredentialID> = db
            .list_webauthn_credentials(user_id)
            .await?
            .into_iter()
            .filter_map(|c| {
                let passkey: Passkey = serde_json::from_str(&c.public_key).ok()?;
                Some(passkey.cred_id().clone())
            })
            .collect();

        let (ccr, state) = webauthn
            .start_passkey_registration(
                user_unique_id,
                username,
                username,
                Some(exclude_credentials),
            )
            .map_err(|e| AppError::Internal(anyhow::anyhow!("webauthn start_reg: {e}")))?;

        let state_json = serde_json::to_string(&state)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize state: {e}")))?;

        db.upsert_webauthn_challenge(user_id, "registration", &state_json)
            .await?;

        serde_json::to_value(ccr)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize ccr: {e}")))
    }

    /// Complete passkey registration. Stores the new credential.
    pub async fn finish_registration(
        webauthn: &Webauthn,
        db: &Arc<dyn Store>,
        user_id: i64,
        name: &str,
        response: serde_json::Value,
    ) -> Result<WebauthnCredential, AppError> {
        let challenge_row = db
            .find_webauthn_challenge(user_id, "registration")
            .await?
            .ok_or_else(|| AppError::BadRequest("no pending registration challenge".into()))?;

        db.delete_webauthn_challenge(user_id, "registration").await?;

        let state: PasskeyRegistration = serde_json::from_str(&challenge_row.challenge)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("deserialize state: {e}")))?;

        let reg_public_key_credential: RegisterPublicKeyCredential =
            serde_json::from_value(response)
                .map_err(|_| AppError::BadRequest("invalid registration response".into()))?;

        let passkey = webauthn
            .finish_passkey_registration(&reg_public_key_credential, &state)
            .map_err(|e| AppError::BadRequest(format!("registration failed: {e}")))?;

        // CredentialID serializes as a base64url string in JSON.
        let cred_id = serde_json::to_value(passkey.cred_id())
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("failed to serialize cred_id")))?;

        let pk_json = serde_json::to_string(&passkey)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize passkey: {e}")))?;

        db.create_webauthn_credential(user_id, &cred_id, &pk_json, name)
            .await
    }

    /// Start passkey authentication for a user. Returns JSON challenge.
    pub async fn start_authentication(
        webauthn: &Webauthn,
        db: &Arc<dyn Store>,
        user_id: i64,
    ) -> Result<serde_json::Value, AppError> {
        let credentials: Vec<Passkey> = db
            .list_webauthn_credentials(user_id)
            .await?
            .into_iter()
            .filter_map(|c| serde_json::from_str(&c.public_key).ok())
            .collect();

        if credentials.is_empty() {
            return Err(AppError::BadRequest("no credentials registered".into()));
        }

        let (rcr, state) = webauthn
            .start_passkey_authentication(&credentials)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("webauthn start_auth: {e}")))?;

        let state_json = serde_json::to_string(&state)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize state: {e}")))?;

        db.upsert_webauthn_challenge(user_id, "authentication", &state_json)
            .await?;

        serde_json::to_value(rcr)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize rcr: {e}")))
    }

    /// Complete passkey authentication. Updates sign count, returns the matched credential row.
    pub async fn finish_authentication(
        webauthn: &Webauthn,
        db: &Arc<dyn Store>,
        user_id: i64,
        response: serde_json::Value,
    ) -> Result<(), AppError> {
        let challenge_row = db
            .find_webauthn_challenge(user_id, "authentication")
            .await?
            .ok_or_else(|| AppError::BadRequest("no pending authentication challenge".into()))?;

        db.delete_webauthn_challenge(user_id, "authentication").await?;

        let state: PasskeyAuthentication = serde_json::from_str(&challenge_row.challenge)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("deserialize state: {e}")))?;

        let auth_gc: PublicKeyCredential = serde_json::from_value(response)
            .map_err(|_| AppError::BadRequest("invalid authentication response".into()))?;

        let result = webauthn
            .finish_passkey_authentication(&auth_gc, &state)
            .map_err(|e| AppError::BadRequest(format!("authentication failed: {e}")))?;

        let cred_id = serde_json::to_value(result.cred_id())
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("failed to serialize cred_id")))?;

        if let Some(cred) = db.find_webauthn_credential_by_cred_id(&cred_id).await? {
            db.update_webauthn_sign_count(cred.id, result.counter() as i64).await?;
        }

        Ok(())
    }
}
