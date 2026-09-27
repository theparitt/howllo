-- The quickstart is the only install path that publishes a sample board.
-- Normal workspaces still start unpublished and without boards.
WITH demo AS (
    SELECT id FROM tenants
    WHERE name = 'Howllo Demo' AND slug LIKE 'howllo-demo-%'
    ORDER BY created_at LIMIT 1
)
UPDATE tenants SET is_published = TRUE, updated_at = NOW()
WHERE id IN (SELECT id FROM demo);

INSERT INTO tenant_branding (tenant_id, site_name, show_boards, show_feed, show_roadmap)
SELECT id, 'Howllo Demo', TRUE, FALSE, FALSE
FROM tenants WHERE name = 'Howllo Demo' AND slug LIKE 'howllo-demo-%'
ORDER BY created_at LIMIT 1
ON CONFLICT (tenant_id) DO NOTHING;

INSERT INTO boards (
    tenant_id, slug, name, description, board_type, is_private, is_default,
    is_enabled, first_enabled_at, intro_text, allow_comments, allow_votes
)
SELECT id, 'general', 'Community', 'Ideas, questions, and product discussion.',
       'discussions', FALSE, FALSE, TRUE, NOW(),
       'Welcome to your first Howllo board.', TRUE, TRUE
FROM tenants WHERE name = 'Howllo Demo' AND slug LIKE 'howllo-demo-%'
ORDER BY created_at LIMIT 1
ON CONFLICT (tenant_id, slug) DO NOTHING;
