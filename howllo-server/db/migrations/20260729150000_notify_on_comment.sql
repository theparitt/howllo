-- Followers can be notified of new comments too. Authors follow their own
-- posts so "my posts" light up. See docs/ACTIVITY_AND_INVITATIONS.md §2.

ALTER TABLE post_follows
    ADD COLUMN IF NOT EXISTS notify_on_comment BOOLEAN NOT NULL DEFAULT TRUE;

-- Backfill: every author follows their own existing posts (idempotent).
INSERT INTO post_follows (post_id, user_id)
SELECT id, user_id FROM posts
ON CONFLICT (post_id, user_id) DO NOTHING;
