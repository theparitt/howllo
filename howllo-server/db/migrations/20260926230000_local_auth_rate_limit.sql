-- Shared local-auth limits keyed by SHA-256 of an IP or account identifier.
-- Rows expire logically after a minute; no raw IP or username is stored here.
CREATE TABLE local_auth_rate_limits (
    scope TEXT NOT NULL,
    identity_hash TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    attempts INTEGER NOT NULL CHECK (attempts > 0),
    PRIMARY KEY (scope, identity_hash)
);
