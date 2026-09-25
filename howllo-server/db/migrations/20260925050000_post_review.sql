ALTER TABLE tenant_branding
    ADD COLUMN IF NOT EXISTS require_post_approval BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE posts
    ADD COLUMN IF NOT EXISTS review_state TEXT NOT NULL DEFAULT 'approved';

ALTER TABLE posts
    ADD COLUMN IF NOT EXISTS review_reason TEXT;

ALTER TABLE posts
    ADD CONSTRAINT posts_review_state_check CHECK (review_state IN ('approved', 'pending', 'rejected'));

CREATE INDEX IF NOT EXISTS posts_author_workspace_created_idx
    ON posts (tenant_id, user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS posts_pending_review_idx
    ON posts (tenant_id, created_at DESC) WHERE review_state = 'pending';
