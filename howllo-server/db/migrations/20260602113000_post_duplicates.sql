ALTER TABLE posts
ADD COLUMN duplicate_of_post_id UUID REFERENCES posts(id) ON DELETE SET NULL;
