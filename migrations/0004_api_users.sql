-- People allowed to call the API. Each has one bearer token, stored only as a
-- SHA-256 hex digest; the plaintext is shown once, when it's issued.
CREATE TABLE api_users (
    id             INTEGER PRIMARY KEY,
    name           TEXT NOT NULL UNIQUE COLLATE NOCASE,
    token_hash     TEXT NOT NULL UNIQUE,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    last_active_at TEXT                              -- last authenticated request
) STRICT;
