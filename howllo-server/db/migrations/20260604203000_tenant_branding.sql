CREATE TABLE IF NOT EXISTS tenant_branding (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    site_name VARCHAR(255),
    logo_url TEXT,
    accent_color VARCHAR(32),
    show_powered_by BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
