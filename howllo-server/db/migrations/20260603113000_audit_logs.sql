CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    entity_type VARCHAR(64) NOT NULL,
    entity_id UUID NOT NULL,
    action VARCHAR(64) NOT NULL,
    old_value JSONB,
    new_value JSONB,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS audit_logs_tenant_created_at_idx
    ON audit_logs (tenant_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS audit_logs_actor_created_at_idx
    ON audit_logs (actor_user_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS audit_logs_entity_created_at_idx
    ON audit_logs (entity_type, entity_id, created_at DESC, id DESC);
