ALTER TABLE workspace_invitations
    ADD COLUMN redemption_code_hash TEXT UNIQUE;
