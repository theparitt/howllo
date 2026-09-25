-- Workspace-wide participation restrictions for public members.
CREATE TABLE workspace_restrictions (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('suspended', 'banned')),
    reason TEXT NOT NULL DEFAULT '',
    expires_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, user_id),
    CONSTRAINT workspace_restrictions_expiry_check CHECK (
        (kind = 'banned' AND expires_at IS NULL) OR
        (kind = 'suspended' AND expires_at IS NOT NULL)
    )
);

CREATE INDEX workspace_restrictions_active_idx
    ON workspace_restrictions (tenant_id, expires_at);
