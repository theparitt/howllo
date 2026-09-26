CREATE TABLE workspace_customer_auth (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    local_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    rooiam_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    rooiam_workspace_id TEXT,
    rooiam_client_id TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE workspace_oidc_providers (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    provider_key TEXT NOT NULL,
    display_name TEXT NOT NULL,
    issuer TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_secret_ciphertext TEXT NOT NULL,
    token_endpoint_auth_method TEXT NOT NULL DEFAULT 'client_secret_post',
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, provider_key)
);

ALTER TABLE auth_exchange_codes ADD COLUMN tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE;
ALTER TABLE auth_exchange_codes ADD COLUMN auth_provider TEXT;
ALTER TABLE account_sessions ADD COLUMN tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE;
ALTER TABLE account_sessions ADD COLUMN auth_provider TEXT NOT NULL DEFAULT 'local';
