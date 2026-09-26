-- Keep draft boards distinct from boards that were enabled and later paused.
ALTER TABLE boards ADD COLUMN first_enabled_at TIMESTAMPTZ;
UPDATE boards SET first_enabled_at = COALESCE(updated_at, created_at) WHERE is_enabled = TRUE;
