-- One operator account: a shared counter prevents brute-force guesses across
-- API replicas. It deliberately does not trust client-supplied proxy headers.
CREATE TABLE admin_auth_rate_limit (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),
    window_start TIMESTAMPTZ NOT NULL,
    attempts INTEGER NOT NULL CHECK (attempts > 0)
);
