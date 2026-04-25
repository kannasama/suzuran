CREATE TABLE user_preferences (
    user_id  INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key      TEXT    NOT NULL,
    value    TEXT    NOT NULL,
    PRIMARY KEY (user_id, key)
);
