CREATE UNIQUE INDEX IF NOT EXISTS webauthn_challenges_user_kind_uq
    ON webauthn_challenges(user_id, kind);
