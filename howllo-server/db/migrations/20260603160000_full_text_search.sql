ALTER TABLE posts ADD COLUMN IF NOT EXISTS search_vector tsvector;

CREATE INDEX IF NOT EXISTS idx_posts_search_fts ON posts USING GIN (search_vector);

CREATE OR REPLACE FUNCTION posts_search_update() RETURNS trigger AS $$
BEGIN
    NEW.search_vector := to_tsvector('english', coalesce(NEW.title, '') || ' ' || coalesce(NEW.body, ''));
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_posts_search_update ON posts;
CREATE TRIGGER trg_posts_search_update
    BEFORE INSERT OR UPDATE OF title, body ON posts
    FOR EACH ROW EXECUTE FUNCTION posts_search_update();

UPDATE posts SET search_vector = to_tsvector('english', coalesce(title, '') || ' ' || coalesce(body, ''));
