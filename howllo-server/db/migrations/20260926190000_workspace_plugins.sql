-- Plugin packages are shipped with Howllo. A platform admin controls which
-- packages are approved; workspace owners only enable approved packages.
CREATE TABLE plugin_catalog (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    slot TEXT NOT NULL CHECK (slot IN ('workspace.typography', 'board.directory.layout')),
    stylesheet_path TEXT NOT NULL,
    is_approved BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE workspace_plugins (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL REFERENCES plugin_catalog(id) ON DELETE CASCADE,
    slot TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, plugin_id)
);
CREATE UNIQUE INDEX workspace_plugins_one_active_per_slot
    ON workspace_plugins (tenant_id, slot) WHERE enabled = TRUE;

INSERT INTO plugin_catalog (id, version, name, description, slot, stylesheet_path, is_approved) VALUES
    ('editorial-type', '1.0.0', 'Editorial type', 'A serif reading style for the public board.', 'workspace.typography', '/plugins/editorial-type.css', TRUE),
    ('clean-type', '1.0.0', 'Clean type', 'A compact system font for dense feedback.', 'workspace.typography', '/plugins/clean-type.css', TRUE),
    ('board-grid', '1.0.0', 'Board grid', 'Show the board directory as a responsive card grid.', 'board.directory.layout', '/plugins/board-grid.css', TRUE);
