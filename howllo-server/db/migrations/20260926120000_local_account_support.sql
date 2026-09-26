ALTER TABLE account_sessions
    ADD COLUMN login_ip INET,
    ADD COLUMN user_agent VARCHAR(512);

CREATE INDEX account_sessions_recent_by_user_idx ON account_sessions (user_id, created_at DESC);

CREATE TABLE local_recovery_issuances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    issued_by UUID NOT NULL REFERENCES users(id),
    reason VARCHAR(500) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
