use chrono::{DateTime, Utc};

use crate::{dal::{Store, UpsertTrack}, error::AppError, models::{Job, Library, OrganizationRule, Session, Setting, TagSuggestion, Theme, TotpEntry, Track, UpsertTagSuggestion, User, WebauthnChallenge, WebauthnCredential}};
use sqlx::PgPool;

pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("migrations/postgres").run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl Store for PgStore {
    async fn health_check(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn count_users(&self) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    async fn create_user(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &str,
    ) -> Result<User, AppError> {
        sqlx::query_as::<_, User>(
            "INSERT INTO users (username, email, password_hash, role)
             VALUES ($1, $2, $3, $4)
             RETURNING *",
        )
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(role)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db) if db.constraint() == Some("users_username_key") => {
                AppError::BadRequest("username already taken".into())
            }
            sqlx::Error::Database(ref db) if db.constraint() == Some("users_email_key") => {
                AppError::BadRequest("email already registered".into())
            }
            other => AppError::Database(other),
        })
    }

    async fn find_user_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_user_by_id(&self, id: i64) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Session, AppError> {
        sqlx::query_as::<_, Session>(
            "INSERT INTO sessions (user_id, token_hash, expires_at)
             VALUES ($1, $2, $3)
             RETURNING *",
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_session_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Session>, AppError> {
        sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn delete_session(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn update_session_token_hash(
        &self,
        session_id: i64,
        token_hash: &str,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET token_hash = $1 WHERE id = $2")
            .bind(token_hash)
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn create_totp_entry(&self, user_id: i64, secret: &str) -> Result<TotpEntry, AppError> {
        sqlx::query_as::<_, TotpEntry>(
            "INSERT INTO totp_entries (user_id, secret) VALUES ($1, $2)
             ON CONFLICT (user_id) DO UPDATE SET secret = $2, verified = FALSE
             RETURNING *",
        )
        .bind(user_id)
        .bind(secret)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_totp_entry(&self, user_id: i64) -> Result<Option<TotpEntry>, AppError> {
        sqlx::query_as::<_, TotpEntry>("SELECT * FROM totp_entries WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn mark_totp_verified(&self, user_id: i64) -> Result<(), AppError> {
        sqlx::query("UPDATE totp_entries SET verified = TRUE WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn delete_totp_entry(&self, user_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM totp_entries WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn create_webauthn_credential(
        &self,
        user_id: i64,
        credential_id: &str,
        public_key: &str,
        name: &str,
    ) -> Result<WebauthnCredential, AppError> {
        sqlx::query_as::<_, WebauthnCredential>(
            "INSERT INTO webauthn_credentials (user_id, credential_id, public_key, name)
             VALUES ($1, $2, $3, $4)
             RETURNING *",
        )
        .bind(user_id)
        .bind(credential_id)
        .bind(public_key)
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn list_webauthn_credentials(&self, user_id: i64) -> Result<Vec<WebauthnCredential>, AppError> {
        sqlx::query_as::<_, WebauthnCredential>(
            "SELECT * FROM webauthn_credentials WHERE user_id = $1 ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_webauthn_credential_by_cred_id(
        &self,
        credential_id: &str,
    ) -> Result<Option<WebauthnCredential>, AppError> {
        sqlx::query_as::<_, WebauthnCredential>(
            "SELECT * FROM webauthn_credentials WHERE credential_id = $1",
        )
        .bind(credential_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn update_webauthn_sign_count(&self, id: i64, sign_count: i64) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE webauthn_credentials SET sign_count = $1, last_used_at = NOW() WHERE id = $2",
        )
        .bind(sign_count)
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
    }

    async fn delete_webauthn_credential(&self, id: i64, user_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM webauthn_credentials WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn upsert_webauthn_challenge(
        &self,
        user_id: i64,
        kind: &str,
        challenge: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO webauthn_challenges (user_id, kind, challenge)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id, kind)
             DO UPDATE SET challenge = $3, created_at = NOW()",
        )
        .bind(user_id)
        .bind(kind)
        .bind(challenge)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
    }

    async fn find_webauthn_challenge(
        &self,
        user_id: i64,
        kind: &str,
    ) -> Result<Option<WebauthnChallenge>, AppError> {
        sqlx::query_as::<_, WebauthnChallenge>(
            "SELECT * FROM webauthn_challenges WHERE user_id = $1 AND kind = $2",
        )
        .bind(user_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn delete_webauthn_challenge(&self, user_id: i64, kind: &str) -> Result<(), AppError> {
        sqlx::query(
            "DELETE FROM webauthn_challenges WHERE user_id = $1 AND kind = $2",
        )
        .bind(user_id)
        .bind(kind)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
    }

    async fn get_setting(&self, key: &str) -> Result<Option<Setting>, AppError> {
        sqlx::query_as::<_, Setting>("SELECT * FROM settings WHERE key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_all_settings(&self) -> Result<Vec<Setting>, AppError> {
        sqlx::query_as::<_, Setting>("SELECT * FROM settings ORDER BY key")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn set_setting(&self, key: &str, value: &str) -> Result<Setting, AppError> {
        sqlx::query_as::<_, Setting>(
            "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, NOW())
             ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()
             RETURNING *",
        )
        .bind(key)
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn list_themes(&self) -> Result<Vec<Theme>, AppError> {
        sqlx::query_as::<_, Theme>("SELECT * FROM themes ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_theme(&self, id: i64) -> Result<Option<Theme>, AppError> {
        sqlx::query_as::<_, Theme>("SELECT * FROM themes WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn create_theme(
        &self,
        name: &str,
        css_vars: serde_json::Value,
        accent_color: Option<&str>,
        background_url: Option<&str>,
    ) -> Result<Theme, AppError> {
        sqlx::query_as::<_, Theme>(
            "INSERT INTO themes (name, css_vars, accent_color, background_url)
             VALUES ($1, $2, $3, $4)
             RETURNING *",
        )
        .bind(name)
        .bind(css_vars)
        .bind(accent_color)
        .bind(background_url)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db) if db.constraint() == Some("themes_name_key") => {
                AppError::BadRequest("theme name already exists".into())
            }
            other => AppError::Database(other),
        })
    }

    async fn update_theme(
        &self,
        id: i64,
        name: &str,
        css_vars: serde_json::Value,
        accent_color: Option<&str>,
        background_url: Option<&str>,
    ) -> Result<Option<Theme>, AppError> {
        sqlx::query_as::<_, Theme>(
            "UPDATE themes SET name=$1, css_vars=$2, accent_color=$3, background_url=$4
             WHERE id=$5
             RETURNING *",
        )
        .bind(name)
        .bind(css_vars)
        .bind(accent_color)
        .bind(background_url)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn delete_theme(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM themes WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn list_libraries(&self) -> Result<Vec<Library>, AppError> {
        sqlx::query_as::<_, Library>("SELECT * FROM libraries ORDER BY name")
            .fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_library(&self, id: i64) -> Result<Option<Library>, AppError> {
        sqlx::query_as::<_, Library>("SELECT * FROM libraries WHERE id = $1")
            .bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn create_library(
        &self, name: &str, root_path: &str, format: &str, parent_library_id: Option<i64>,
    ) -> Result<Library, AppError> {
        sqlx::query_as::<_, Library>(
            "INSERT INTO libraries (name, root_path, format, parent_library_id)
             VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(name).bind(root_path).bind(format).bind(parent_library_id)
        .fetch_one(&self.pool).await.map_err(AppError::Database)
    }

    async fn update_library(
        &self, id: i64, name: &str, scan_enabled: bool, scan_interval_secs: i64,
        auto_transcode_on_ingest: bool, auto_organize_on_ingest: bool,
    ) -> Result<Option<Library>, AppError> {
        sqlx::query_as::<_, Library>(
            "UPDATE libraries SET name=$1, scan_enabled=$2, scan_interval_secs=$3,
             auto_transcode_on_ingest=$4, auto_organize_on_ingest=$5
             WHERE id=$6 RETURNING *",
        )
        .bind(name).bind(scan_enabled).bind(scan_interval_secs)
        .bind(auto_transcode_on_ingest).bind(auto_organize_on_ingest).bind(id)
        .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn delete_library(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM libraries WHERE id = $1")
            .bind(id).execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
    }

    async fn list_organization_rules(&self, library_id: Option<i64>) -> Result<Vec<OrganizationRule>, AppError> {
        let rows = if let Some(lid) = library_id {
            sqlx::query_as::<_, OrganizationRule>(
                "SELECT * FROM organization_rules
                 WHERE library_id IS NULL OR library_id = $1
                 ORDER BY priority ASC",
            )
            .bind(lid)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)?
        } else {
            sqlx::query_as::<_, OrganizationRule>(
                "SELECT * FROM organization_rules ORDER BY priority ASC",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)?
        };
        Ok(rows)
    }

    async fn get_organization_rule(&self, id: i64) -> Result<Option<OrganizationRule>, AppError> {
        sqlx::query_as::<_, OrganizationRule>("SELECT * FROM organization_rules WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn create_organization_rule(
        &self,
        name: &str,
        library_id: Option<i64>,
        priority: i32,
        conditions: Option<serde_json::Value>,
        path_template: &str,
        enabled: bool,
    ) -> Result<OrganizationRule, AppError> {
        sqlx::query_as::<_, OrganizationRule>(
            "INSERT INTO organization_rules (name, library_id, priority, conditions, path_template, enabled)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
        )
        .bind(name)
        .bind(library_id)
        .bind(priority)
        .bind(conditions)
        .bind(path_template)
        .bind(enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn update_organization_rule(
        &self,
        id: i64,
        name: &str,
        priority: i32,
        conditions: Option<serde_json::Value>,
        path_template: &str,
        enabled: bool,
    ) -> Result<Option<OrganizationRule>, AppError> {
        sqlx::query_as::<_, OrganizationRule>(
            "UPDATE organization_rules
             SET name=$1, priority=$2, conditions=$3, path_template=$4, enabled=$5
             WHERE id=$6 RETURNING *",
        )
        .bind(name)
        .bind(priority)
        .bind(conditions)
        .bind(path_template)
        .bind(enabled)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn delete_organization_rule(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM organization_rules WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn list_tracks_by_library(&self, library_id: i64) -> Result<Vec<Track>, AppError> {
        sqlx::query_as::<_, Track>(
            "SELECT * FROM tracks WHERE library_id = $1 ORDER BY albumartist, album, discnumber, tracknumber",
        )
        .bind(library_id).fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_track(&self, id: i64) -> Result<Option<Track>, AppError> {
        sqlx::query_as::<_, Track>("SELECT * FROM tracks WHERE id = $1")
            .bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn find_track_by_path(&self, library_id: i64, relative_path: &str) -> Result<Option<Track>, AppError> {
        sqlx::query_as::<_, Track>(
            "SELECT * FROM tracks WHERE library_id = $1 AND relative_path = $2",
        )
        .bind(library_id).bind(relative_path)
        .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn upsert_track(&self, t: UpsertTrack) -> Result<Track, AppError> {
        sqlx::query_as::<_, Track>(
            "INSERT INTO tracks (library_id, relative_path, file_hash, title, artist, albumartist,
             album, tracknumber, discnumber, totaldiscs, totaltracks, date, genre, composer,
             label, catalognumber, tags, duration_secs, bitrate, sample_rate, channels, bit_depth,
             has_embedded_art, last_scanned_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,NOW())
             ON CONFLICT (library_id, relative_path) DO UPDATE SET
               file_hash=$3, title=$4, artist=$5, albumartist=$6, album=$7, tracknumber=$8,
               discnumber=$9, totaldiscs=$10, totaltracks=$11, date=$12, genre=$13, composer=$14,
               label=$15, catalognumber=$16, tags=$17, duration_secs=$18, bitrate=$19,
               sample_rate=$20, channels=$21, bit_depth=$22, has_embedded_art=$23,
               last_scanned_at=NOW()
             RETURNING *",
        )
        .bind(t.library_id).bind(&t.relative_path).bind(&t.file_hash)
        .bind(&t.title).bind(&t.artist).bind(&t.albumartist).bind(&t.album)
        .bind(&t.tracknumber).bind(&t.discnumber).bind(&t.totaldiscs).bind(&t.totaltracks)
        .bind(&t.date).bind(&t.genre).bind(&t.composer).bind(&t.label).bind(&t.catalognumber)
        .bind(&t.tags).bind(t.duration_secs).bind(t.bitrate).bind(t.sample_rate)
        .bind(t.channels).bind(t.bit_depth).bind(t.has_embedded_art)
        .fetch_one(&self.pool).await.map_err(AppError::Database)
    }

    async fn mark_track_removed(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM tracks WHERE id = $1")
            .bind(id).execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
    }

    async fn list_track_paths_by_library(&self, library_id: i64) -> Result<Vec<(i64, String, String)>, AppError> {
        sqlx::query_as::<_, (i64, String, String)>(
            "SELECT id, relative_path, file_hash FROM tracks WHERE library_id = $1",
        )
        .bind(library_id).fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn update_track_path(&self, id: i64, relative_path: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE tracks SET relative_path = $1 WHERE id = $2")
            .bind(relative_path)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn update_track_fingerprint(
        &self,
        track_id: i64,
        fingerprint: &str,
        duration_secs: f64,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"UPDATE tracks
               SET tags = tags || jsonb_build_object('acoustid_fingerprint', $1::text),
                   duration_secs = $2,
                   acoustid_fingerprint = $1
               WHERE id = $3"#,
        )
        .bind(fingerprint)
        .bind(duration_secs)
        .bind(track_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
    }

    async fn enqueue_job(
        &self,
        job_type: &str,
        payload: serde_json::Value,
        priority: i64,
    ) -> Result<Job, AppError> {
        sqlx::query_as::<_, Job>(
            "INSERT INTO jobs (job_type, payload, priority) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(job_type).bind(payload).bind(priority)
        .fetch_one(&self.pool).await.map_err(AppError::Database)
    }

    async fn claim_next_job(&self, job_types: &[&str]) -> Result<Option<Job>, AppError> {
        sqlx::query_as::<_, Job>(
            "UPDATE jobs SET status = 'running', started_at = NOW(), attempts = attempts + 1
             WHERE id = (
                 SELECT id FROM jobs
                 WHERE status = 'pending'
                   AND job_type = ANY($1)
                 ORDER BY priority DESC, created_at ASC
                 LIMIT 1
                 FOR UPDATE SKIP LOCKED
             )
             RETURNING *",
        )
        .bind(job_types)
        .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn complete_job(&self, id: i64, result: serde_json::Value) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE jobs SET status='completed', result=$1, completed_at=NOW() WHERE id=$2",
        )
        .bind(result).bind(id)
        .execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
    }

    async fn fail_job(&self, id: i64, error: &str) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE jobs SET
               status = CASE WHEN attempts >= 3 THEN 'failed' ELSE 'pending' END,
               error = $1,
               started_at = NULL
             WHERE id = $2",
        )
        .bind(error).bind(id)
        .execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
    }

    async fn cancel_job(&self, id: i64) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE jobs SET status='cancelled' WHERE id=$1 AND status IN ('pending','running')",
        )
        .bind(id)
        .execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
    }

    async fn list_jobs(&self, status: Option<&str>, limit: i64) -> Result<Vec<Job>, AppError> {
        if let Some(s) = status {
            sqlx::query_as::<_, Job>(
                "SELECT * FROM jobs WHERE status=$1 ORDER BY created_at DESC LIMIT $2",
            )
            .bind(s).bind(limit)
            .fetch_all(&self.pool).await.map_err(AppError::Database)
        } else {
            sqlx::query_as::<_, Job>(
                "SELECT * FROM jobs ORDER BY created_at DESC LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&self.pool).await.map_err(AppError::Database)
        }
    }

    async fn get_job(&self, id: i64) -> Result<Option<Job>, AppError> {
        sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE id = $1")
            .bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    // ── tag suggestions ───────────────────────────────────────────

    async fn create_tag_suggestion(&self, dto: UpsertTagSuggestion) -> Result<TagSuggestion, AppError> {
        sqlx::query_as::<_, TagSuggestion>(
            "INSERT INTO tag_suggestions
             (track_id, source, suggested_tags, confidence, mb_recording_id, mb_release_id, cover_art_url)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING *",
        )
        .bind(dto.track_id)
        .bind(dto.source)
        .bind(dto.suggested_tags)
        .bind(dto.confidence)
        .bind(dto.mb_recording_id)
        .bind(dto.mb_release_id)
        .bind(dto.cover_art_url)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn list_pending_tag_suggestions(&self, track_id: Option<i64>) -> Result<Vec<TagSuggestion>, AppError> {
        sqlx::query_as::<_, TagSuggestion>(
            "SELECT * FROM tag_suggestions
             WHERE status = 'pending'
               AND ($1::bigint IS NULL OR track_id = $1)
             ORDER BY confidence DESC, created_at ASC",
        )
        .bind(track_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn get_tag_suggestion(&self, id: i64) -> Result<Option<TagSuggestion>, AppError> {
        sqlx::query_as::<_, TagSuggestion>("SELECT * FROM tag_suggestions WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn set_tag_suggestion_status(&self, id: i64, status: &str) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE tag_suggestions SET status = $1 WHERE id = $2 AND status = 'pending'",
        )
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;
        if result.rows_affected() == 0 {
            // Distinguish: row missing vs. row exists but already resolved
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tag_suggestions WHERE id = $1)")
                    .bind(id)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(AppError::Database)?;
            if exists {
                return Err(AppError::Conflict(format!(
                    "tag_suggestion {id} is not pending"
                )));
            } else {
                return Err(AppError::NotFound(format!("tag_suggestion {id}")));
            }
        }
        Ok(())
    }

    async fn pending_tag_suggestion_count(&self) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tag_suggestions WHERE status = 'pending'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)?;
        Ok(row.0)
    }

    async fn update_track_tags(&self, track_id: i64, tags: serde_json::Value) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"UPDATE tracks SET
                 tags          = $1,
                 title         = ($1 ->> 'title'),
                 artist        = ($1 ->> 'artist'),
                 albumartist   = ($1 ->> 'albumartist'),
                 album         = ($1 ->> 'album'),
                 date          = ($1 ->> 'date'),
                 genre         = ($1 ->> 'genre'),
                 tracknumber   = ($1 ->> 'tracknumber'),
                 discnumber    = ($1 ->> 'discnumber'),
                 label         = ($1 ->> 'label'),
                 catalognumber = ($1 ->> 'catalognumber')
               WHERE id = $2"#,
        )
        .bind(tags)
        .bind(track_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("track {track_id}")));
        }
        Ok(())
    }
}
