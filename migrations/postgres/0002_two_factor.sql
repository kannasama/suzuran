-- migrations/postgres/0002_two_factor.sql

CREATE TABLE totp_entries (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    secret      TEXT NOT NULL,  -- encrypted TOTP secret
    verified    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE webauthn_credentials (
    id              BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_id   TEXT NOT NULL UNIQUE,
    public_key      TEXT NOT NULL,  -- CBOR-encoded public key, base64
    sign_count      BIGINT NOT NULL DEFAULT 0,
    name            TEXT NOT NULL DEFAULT 'Security Key',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at    TIMESTAMPTZ
);

CREATE INDEX webauthn_credentials_user_id_idx ON webauthn_credentials(user_id);
CREATE INDEX webauthn_credentials_credential_id_idx ON webauthn_credentials(credential_id);

-- Stores in-flight WebAuthn challenge state (short-lived, cleaned up on completion)
CREATE TABLE webauthn_challenges (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    challenge   TEXT NOT NULL,  -- JSON-serialized PasskeyRegistration or PasskeyAuthentication state
    kind        TEXT NOT NULL CHECK (kind IN ('registration', 'authentication')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX webauthn_challenges_user_id_idx ON webauthn_challenges(user_id);
