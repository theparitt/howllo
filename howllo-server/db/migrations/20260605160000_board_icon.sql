-- Per-board icon image. Public URL of an uploaded image (via /api/uploads),
-- shown next to the board name in the admin and the public web app. Null means
-- "no icon" — the UI falls back to a generated letter avatar.
ALTER TABLE boards
    ADD COLUMN icon_url TEXT;
