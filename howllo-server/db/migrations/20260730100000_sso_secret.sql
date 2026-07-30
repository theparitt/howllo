-- Per-workspace SSO shared secret for end-user token-exchange. The customer's
-- backend signs a JWT with this secret; howllo verifies it and mints a session.
-- NULL = SSO disabled for the workspace. This value is sensitive and is NEVER
-- returned by the public /api/workspace-auth endpoint — only to owners/admins.
ALTER TABLE workspace_auth_configs
    ADD COLUMN IF NOT EXISTS sso_secret TEXT;
