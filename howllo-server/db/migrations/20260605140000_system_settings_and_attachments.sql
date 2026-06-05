-- Platform runtime settings + post attachments.
--
-- system_settings: a generic key/value store for operator-tunable platform
-- config (storage backend, MinIO connection, etc). The app reads a value from
-- this table first; if absent/empty it falls back to the HOWLLO_* env var. This
-- lets an admin override env defaults at runtime via "test & save" in the admin
-- panel — same pattern as rooiam.
CREATE TABLE system_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL DEFAULT '',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Public URLs of images a user attached to a post (e.g. screenshots). Stored as
-- a JSON array of strings. Empty array when none.
ALTER TABLE posts
    ADD COLUMN attachments JSONB NOT NULL DEFAULT '[]'::jsonb;
