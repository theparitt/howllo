ALTER TABLE tenant_branding
    ADD COLUMN posts_per_hour INT NOT NULL DEFAULT 3 CHECK (posts_per_hour BETWEEN 1 AND 100),
    ADD COLUMN comments_per_hour INT NOT NULL DEFAULT 15 CHECK (comments_per_hour BETWEEN 1 AND 300),
    ADD COLUMN board_posts_per_10m INT NOT NULL DEFAULT 20 CHECK (board_posts_per_10m BETWEEN 1 AND 500),
    ADD COLUMN board_comments_per_10m INT NOT NULL DEFAULT 60 CHECK (board_comments_per_10m BETWEEN 1 AND 1000);

CREATE TABLE board_write_cooldowns (
    board_id UUID NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('post', 'comment')),
    until_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (board_id, kind)
);

CREATE TABLE workspace_spam_flags (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    until_at TIMESTAMPTZ NOT NULL,
    reason TEXT NOT NULL,
    PRIMARY KEY (tenant_id, user_id)
);

CREATE INDEX comments_post_created_idx ON comments (post_id, created_at DESC);
