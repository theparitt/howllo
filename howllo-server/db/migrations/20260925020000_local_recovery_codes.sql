-- One active, high-entropy recovery code per local account. Only its digest
-- is stored. Replacing or consuming the code invalidates the previous one.
CREATE TABLE local_recovery_codes (
    user_id UUID PRIMARY KEY REFERENCES local_credentials(user_id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
