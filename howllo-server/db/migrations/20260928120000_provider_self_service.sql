ALTER TABLE workspace_sessions
  ADD COLUMN IF NOT EXISTS provider_access_ciphertext TEXT,
  ADD COLUMN IF NOT EXISTS provider_refresh_ciphertext TEXT,
  ADD COLUMN IF NOT EXISTS provider_access_expires_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS provider_session_id UUID,
  ADD COLUMN IF NOT EXISTS provider_token_pending_validation BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS workspace_sessions_provider_session_idx
  ON workspace_sessions (user_id, provider_session_id)
  WHERE provider_session_id IS NOT NULL AND revoked_at IS NULL;
