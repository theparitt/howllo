ALTER TABLE boards
    ADD COLUMN intro_text TEXT,
    ADD COLUMN allow_votes BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN allow_comments BOOLEAN NOT NULL DEFAULT TRUE;

ALTER TABLE boards
    ADD CONSTRAINT boards_intro_text_length CHECK (intro_text IS NULL OR char_length(intro_text) <= 240);
