ALTER TABLE webauthn_challenges
    ADD CONSTRAINT webauthn_challenges_user_kind_uq UNIQUE (user_id, kind);
