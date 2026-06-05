ALTER TABLE audit_logs
ADD COLUMN IF NOT EXISTS request_id VARCHAR(100);

ALTER TABLE api_tokens
ADD COLUMN IF NOT EXISTS scopes TEXT[] NOT NULL DEFAULT ARRAY['posts:read']::TEXT[];

ALTER TABLE webhook_deliveries
ADD COLUMN IF NOT EXISTS attempt_count INT NOT NULL DEFAULT 1;

CREATE TABLE IF NOT EXISTS ai_suggestions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    post_id UUID REFERENCES posts(id) ON DELETE CASCADE,
    suggestion_type VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    payload JSONB NOT NULL,
    created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    reviewed_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ai_suggestions_status_check CHECK (status IN ('pending', 'accepted', 'rejected'))
);

CREATE INDEX IF NOT EXISTS ai_suggestions_tenant_created_at_idx
    ON ai_suggestions (tenant_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_posts_search_fts
    ON posts
    USING GIN (to_tsvector('simple', coalesce(title, '') || ' ' || coalesce(body, '')));
