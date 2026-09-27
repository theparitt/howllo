-- Share public-write IP limits across API workers and replicas. Store only a
-- SHA-256 digest of the client IP; old windows are removed opportunistically.
CREATE TABLE public_write_rate_limits (
    scope TEXT NOT NULL,
    identity_hash TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    attempts INTEGER NOT NULL CHECK (attempts > 0),
    PRIMARY KEY (scope, identity_hash)
);

CREATE INDEX public_write_rate_limits_window_idx
    ON public_write_rate_limits (window_start);
