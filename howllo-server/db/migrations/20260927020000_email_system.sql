-- Platform owns transport and hard budgets. Workspace choices never expose SMTP secrets.
CREATE TABLE email_platform_settings (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    smtp_host TEXT NOT NULL DEFAULT '',
    smtp_port INTEGER NOT NULL DEFAULT 587 CHECK (smtp_port BETWEEN 1 AND 65535),
    smtp_tls_mode TEXT NOT NULL DEFAULT 'starttls' CHECK (smtp_tls_mode IN ('implicit', 'starttls', 'none')),
    smtp_username TEXT NOT NULL DEFAULT '',
    smtp_password_ciphertext TEXT,
    from_email TEXT NOT NULL DEFAULT '',
    from_name TEXT NOT NULL DEFAULT 'Howllo',
    reply_to TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'disabled' CHECK (status IN ('disabled', 'configured', 'tested', 'enabled', 'paused')),
    config_version BIGINT NOT NULL DEFAULT 1,
    tested_version BIGINT,
    tested_at TIMESTAMPTZ,
    failure_count INTEGER NOT NULL DEFAULT 0,
    monthly_limit INTEGER NOT NULL DEFAULT 100000 CHECK (monthly_limit BETWEEN 1 AND 10000000),
    tenant_daily_limit INTEGER NOT NULL DEFAULT 500 CHECK (tenant_daily_limit BETWEEN 1 AND 100000),
    tenant_monthly_limit INTEGER NOT NULL DEFAULT 5000 CHECK (tenant_monthly_limit BETWEEN 1 AND 1000000),
    max_broadcast_recipients INTEGER NOT NULL DEFAULT 1000 CHECK (max_broadcast_recipients BETWEEN 1 AND 100000),
    broadcasts_per_day INTEGER NOT NULL DEFAULT 2 CHECK (broadcasts_per_day BETWEEN 0 AND 100),
    broadcasts_per_week INTEGER NOT NULL DEFAULT 5 CHECK (broadcasts_per_week BETWEEN 0 AND 500),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
INSERT INTO email_platform_settings (id) VALUES (TRUE);

CREATE TABLE tenant_email_settings (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    reply_notifications BOOLEAN NOT NULL DEFAULT TRUE,
    important_updates BOOLEAN NOT NULL DEFAULT TRUE,
    digest_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    broadcast_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    daily_limit INTEGER CHECK (daily_limit IS NULL OR daily_limit BETWEEN 1 AND 100000),
    monthly_limit INTEGER CHECK (monthly_limit IS NULL OR monthly_limit BETWEEN 1 AND 1000000),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE email_suppression (
    recipient TEXT PRIMARY KEY,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE email_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    recipient TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('system', 'notification', 'digest', 'broadcast')),
    category TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    dedupe_key TEXT UNIQUE,
    event_count INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'sending', 'sent', 'failed', 'cancelled')),
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    claimed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at TIMESTAMPTZ,
    last_error TEXT
);
CREATE INDEX email_messages_worker_idx ON email_messages (next_attempt_at, created_at)
    WHERE status = 'queued';
CREATE INDEX email_messages_tenant_usage_idx ON email_messages (tenant_id, created_at)
    WHERE status IN ('queued', 'sending', 'sent');
CREATE INDEX email_messages_platform_usage_idx ON email_messages (created_at)
    WHERE status IN ('queued', 'sending', 'sent');
CREATE INDEX email_messages_recipient_usage_idx ON email_messages (recipient, created_at)
    WHERE status IN ('queued', 'sending', 'sent');

CREATE TABLE user_email_preferences (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    replies_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updates_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    digest_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    broadcast_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (tenant_id, user_id)
);

CREATE TABLE user_verified_emails (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    email TEXT NOT NULL UNIQUE,
    verified_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE email_verification_tokens (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE local_email_reset_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX local_email_reset_tokens_user_idx ON local_email_reset_tokens(user_id,created_at DESC);

CREATE TABLE email_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    message_id UUID REFERENCES email_messages(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    detail TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX email_events_tenant_idx ON email_events(tenant_id,created_at DESC);

CREATE TABLE invitation_email_limits (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    window_start TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    attempts INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, actor_user_id)
);
