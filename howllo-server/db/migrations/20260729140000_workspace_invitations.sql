-- Workspace staff invitations. Owners/admins invite staff by email; the invitee
-- accepts/rejects; the inviter can withdraw. Accept creates a membership.
-- See docs/ACTIVITY_AND_INVITATIONS.md §1.

CREATE TABLE IF NOT EXISTS workspace_invitations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    role VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    invited_by UUID REFERENCES users(id) ON DELETE SET NULL,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    responded_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    CONSTRAINT workspace_invitations_role_check CHECK (role IN ('admin', 'moderator', 'member')),
    CONSTRAINT workspace_invitations_status_check
        CHECK (status IN ('pending', 'accepted', 'rejected', 'withdrawn', 'expired'))
);

-- At most one pending invite per (workspace, email); re-inviting after a
-- reject/withdraw is allowed (history is kept).
CREATE UNIQUE INDEX IF NOT EXISTS workspace_invitations_one_pending_idx
    ON workspace_invitations (tenant_id, lower(email))
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS workspace_invitations_tenant_idx
    ON workspace_invitations (tenant_id, created_at DESC);

CREATE INDEX IF NOT EXISTS workspace_invitations_pending_user_idx
    ON workspace_invitations (user_id) WHERE status = 'pending';
