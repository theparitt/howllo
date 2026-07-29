ALTER TABLE tenant_branding
    ADD COLUMN IF NOT EXISTS background_color VARCHAR(32);

ALTER TABLE boards
    ADD COLUMN IF NOT EXISTS background_color VARCHAR(32);
