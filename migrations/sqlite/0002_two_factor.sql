CREATE TABLE totp_entries (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    secret      TEXT NOT NULL,
    verified    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE webauthn_credentials (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_id   TEXT NOT NULL UNIQUE,
    public_key      TEXT NOT NULL,
    sign_count      INTEGER NOT NULL DEFAULT 0,
    name            TEXT NOT NULL DEFAULT 'Security Key',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at    TEXT
);

CREATE INDEX webauthn_credentials_user_id_idx ON webauthn_credentials(user_id);
CREATE INDEX webauthn_credentials_credential_id_idx ON webauthn_credentials(credential_id);

CREATE TABLE webauthn_challenges (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    challenge   TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('registration', 'authentication')),
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX webauthn_challenges_user_id_idx ON webauthn_challenges(user_id);
