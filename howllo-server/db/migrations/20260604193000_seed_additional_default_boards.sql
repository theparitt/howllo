INSERT INTO boards (tenant_id, slug, name, description, board_type, is_private, is_default)
SELECT
    t.id,
    defaults.slug,
    defaults.name,
    defaults.description,
    defaults.board_type,
    FALSE,
    TRUE
FROM tenants t
CROSS JOIN (
    VALUES
        ('feature-requests', 'Feature Requests', 'Vote on improvements, new capabilities, and product ideas.', 'feedback'),
        ('bug-reports', 'Bug Reports', 'Report broken flows, errors, regressions, and usability problems.', 'support')
) AS defaults(slug, name, description, board_type)
WHERE t.default_board_enabled = TRUE
ON CONFLICT (tenant_id, slug) DO NOTHING;
