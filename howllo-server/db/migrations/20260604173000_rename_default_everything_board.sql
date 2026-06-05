UPDATE boards
SET
    name = 'General',
    description = 'Catch-all board for ideas, bugs, feature requests, and general product discussion.',
    updated_at = NOW()
WHERE is_default = TRUE
  AND slug = 'general'
  AND name = 'Everything';
