-- Existing workspaces stay live; future inserts start as drafts.
ALTER TABLE tenants ADD COLUMN is_published BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE tenants ALTER COLUMN is_published SET DEFAULT FALSE;
