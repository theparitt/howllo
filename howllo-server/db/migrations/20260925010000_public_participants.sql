-- A public board participant may post and vote, but this membership must not
-- grant visibility into private boards in the same workspace.
ALTER TABLE memberships ADD COLUMN public_participant BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX memberships_private_access_idx
    ON memberships (tenant_id, user_id) WHERE public_participant = FALSE;
