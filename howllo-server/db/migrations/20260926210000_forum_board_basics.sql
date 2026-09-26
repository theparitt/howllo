ALTER TABLE posts ADD COLUMN pinned_at TIMESTAMPTZ;
CREATE INDEX posts_board_pinned_idx ON posts (board_id, pinned_at DESC) WHERE pinned_at IS NOT NULL AND is_hidden = FALSE AND deleted_at IS NULL;

CREATE TABLE board_presentation (
    board_id UUID PRIMARY KEY REFERENCES boards(id) ON DELETE CASCADE,
    announcement TEXT NOT NULL DEFAULT '',
    sidebar_text TEXT NOT NULL DEFAULT '',
    footer_text TEXT NOT NULL DEFAULT '',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE board_plugins (
    board_id UUID NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL REFERENCES plugin_catalog(id) ON DELETE CASCADE,
    slot TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (board_id, plugin_id)
);
CREATE UNIQUE INDEX board_plugins_one_active_per_slot ON board_plugins (board_id, slot) WHERE enabled = TRUE;

ALTER TABLE plugin_catalog DROP CONSTRAINT plugin_catalog_slot_check;
ALTER TABLE plugin_catalog ADD CONSTRAINT plugin_catalog_slot_check
    CHECK (slot IN ('workspace.typography', 'board.directory.layout', 'board.topics.layout', 'board.topics.typography'));

INSERT INTO plugin_catalog (id, version, name, description, slot, stylesheet_path, is_approved) VALUES
    ('compact-topics', '1.0.0', 'Compact topics', 'Fit more topics on each page.', 'board.topics.layout', '/plugins/compact-topics.css', TRUE),
    ('reading-type', '1.0.0', 'Reading type', 'Give long discussions a calmer reading style.', 'board.topics.typography', '/plugins/reading-type.css', TRUE);
