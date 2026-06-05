ALTER TABLE comments
ADD COLUMN comment_type VARCHAR(50) NOT NULL DEFAULT 'user';

UPDATE comments
SET comment_type = CASE
    WHEN is_official_response = TRUE THEN 'official'
    ELSE 'user'
END;

CREATE TABLE post_follows (
    post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notify_on_status_change BOOLEAN NOT NULL DEFAULT TRUE,
    notify_on_official_response BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(post_id, user_id)
);
