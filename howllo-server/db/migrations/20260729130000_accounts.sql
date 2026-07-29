-- Account -> Workspaces tenancy. An account (the customer/org) owns many
-- workspaces; `tenants` IS a workspace. See docs/TENANCY.md.
--
-- account_id is left NULLABLE for now: many code paths (and tests) still create
-- bare workspaces, and enforcement can be tightened later once every insert
-- routes through the create-workspace path. Real workspaces are backfilled below.

CREATE TABLE IF NOT EXISTS accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    owner_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS account_memberships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL DEFAULT 'owner',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT account_memberships_role_check CHECK (role IN ('owner', 'admin', 'billing')),
    UNIQUE (account_id, user_id)
);

CREATE INDEX IF NOT EXISTS account_memberships_user_idx ON account_memberships (user_id);

ALTER TABLE tenants
    ADD COLUMN IF NOT EXISTS account_id UUID REFERENCES accounts(id) ON DELETE CASCADE;

-- Backfill: one 1:1 account per existing workspace, owned by that workspace's
-- current `owner` membership (if any). Nothing changes visibly.
DO $$
DECLARE
    t RECORD;
    new_account_id UUID;
    owner_id UUID;
BEGIN
    FOR t IN SELECT id, slug, name FROM tenants WHERE account_id IS NULL LOOP
        SELECT user_id INTO owner_id
            FROM memberships
            WHERE tenant_id = t.id AND role = 'owner'
            LIMIT 1;

        INSERT INTO accounts (slug, name, owner_user_id)
            VALUES (t.slug || '-org', t.name, owner_id)
            RETURNING id INTO new_account_id;

        IF owner_id IS NOT NULL THEN
            INSERT INTO account_memberships (account_id, user_id, role)
                VALUES (new_account_id, owner_id, 'owner')
                ON CONFLICT (account_id, user_id) DO NOTHING;
        END IF;

        UPDATE tenants SET account_id = new_account_id WHERE id = t.id;
    END LOOP;
END $$;
