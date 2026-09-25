CREATE TABLE workspace_policies (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    overrides JSONB NOT NULL DEFAULT '{}'::jsonb,
    storage_cap_mb INT CHECK (storage_cap_mb BETWEEN 1 AND 102400)
);

CREATE TABLE workspace_assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, relative_path)
);

CREATE TABLE workspace_asset_reconciliations (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    reconciled_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX workspace_assets_tenant_idx ON workspace_assets (tenant_id);

-- Preserve settings changed before the policy screen was introduced.
INSERT INTO workspace_policies (tenant_id, overrides)
SELECT tenant_id, jsonb_strip_nulls(jsonb_build_object(
    'posts_per_hour', CASE WHEN posts_per_hour <> 3 THEN posts_per_hour END,
    'comments_per_hour', CASE WHEN comments_per_hour <> 15 THEN comments_per_hour END,
    'board_posts_per_10m', CASE WHEN board_posts_per_10m <> 20 THEN board_posts_per_10m END,
    'board_comments_per_10m', CASE WHEN board_comments_per_10m <> 60 THEN board_comments_per_10m END
))
FROM tenant_branding
WHERE posts_per_hour <> 3 OR comments_per_hour <> 15
   OR board_posts_per_10m <> 20 OR board_comments_per_10m <> 60;
