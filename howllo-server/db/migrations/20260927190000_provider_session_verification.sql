-- Bound the lifetime of a workspace session after provider membership changes.
ALTER TABLE workspace_sessions
ADD COLUMN IF NOT EXISTS provider_verified_at TIMESTAMPTZ;
