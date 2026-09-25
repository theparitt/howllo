-- Preserve user IDs and all existing memberships, posts and votes.
-- The old column stays nullable for a rolling upgrade and legacy API clients.
ALTER TABLE users ALTER COLUMN rooiam_subject DROP NOT NULL;
ALTER TABLE workspace_auth_configs ALTER COLUMN provider SET DEFAULT 'local';

CREATE TABLE user_identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    subject TEXT NOT NULL,
    email TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT user_identities_provider_subject_unique UNIQUE (provider_id, subject),
    CONSTRAINT user_identities_provider_nonempty CHECK (length(provider_id) > 0),
    CONSTRAINT user_identities_subject_nonempty CHECK (length(subject) > 0)
);
CREATE INDEX user_identities_user_id_idx ON user_identities(user_id);

INSERT INTO user_identities (user_id, provider_id, subject, email)
SELECT id,
       CASE WHEN rooiam_subject = 'local-admin' THEN 'local-admin'
            WHEN rooiam_subject LIKE 'sso:%' THEN 'workspace-sso'
            WHEN rooiam_subject LIKE 'invited:%' THEN 'invited'
            ELSE 'rooiam' END,
       rooiam_subject,
       email
FROM users WHERE rooiam_subject IS NOT NULL;

CREATE TABLE local_credentials (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE account_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX account_sessions_user_idx ON account_sessions(user_id);

CREATE TABLE auth_transactions (
    state_hash TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    nonce TEXT NOT NULL,
    code_verifier TEXT NOT NULL,
    return_to TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE auth_exchange_codes (
    code_hash TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    return_to TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
