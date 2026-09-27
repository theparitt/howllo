ALTER TABLE workspace_invitations
    ADD COLUMN provider_id TEXT,
    ADD COLUMN provider_invitation_id UUID;

CREATE UNIQUE INDEX workspace_invitations_provider_invitation_idx
    ON workspace_invitations(provider_id, provider_invitation_id)
    WHERE provider_invitation_id IS NOT NULL;

ALTER TABLE workspace_invitations ADD CONSTRAINT workspace_invitations_provider_pair_check
    CHECK ((provider_id IS NULL) = (provider_invitation_id IS NULL));

-- Remember the chosen identity provider even if the external mapping later
-- disappears. A missing mapping must never silently enable local invitations.
CREATE TABLE workspace_identity_links (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    linked_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
