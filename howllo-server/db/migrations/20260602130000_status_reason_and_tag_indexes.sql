ALTER TABLE post_status_history
ADD COLUMN reason TEXT;

CREATE INDEX idx_posts_board_status_created_at
ON posts (board_id, status, created_at DESC);

CREATE INDEX idx_post_tags_post_id
ON post_tags (post_id);

CREATE INDEX idx_post_tags_tag_id
ON post_tags (tag_id);
