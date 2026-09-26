ALTER TABLE boards
    ADD COLUMN header_image_url TEXT,
    ADD COLUMN background_image_url TEXT;

CREATE TABLE board_categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    board_id UUID NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    slug VARCHAR(64) NOT NULL,
    name VARCHAR(80) NOT NULL,
    color VARCHAR(7) NOT NULL DEFAULT '#64748b' CHECK (color ~ '^#[0-9A-Fa-f]{6}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (board_id, slug),
    UNIQUE (board_id, id)
);

ALTER TABLE posts ADD COLUMN category_id UUID;
ALTER TABLE posts ADD CONSTRAINT posts_category_board_fk
    FOREIGN KEY (board_id, category_id) REFERENCES board_categories(board_id, id);
CREATE INDEX posts_board_category_created_idx ON posts (board_id, category_id, created_at DESC);
