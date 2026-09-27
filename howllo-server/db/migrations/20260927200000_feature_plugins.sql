-- First-party behavior plugins use the same platform approval and per-board
-- enablement lifecycle as appearance plugins. Upload policy is enforced by the
-- API; a CSS asset alone cannot grant a capability.
ALTER TABLE plugin_catalog DROP CONSTRAINT plugin_catalog_slot_check;
ALTER TABLE plugin_catalog ADD CONSTRAINT plugin_catalog_slot_check CHECK (slot IN (
    'workspace.typography', 'board.directory.layout',
    'board.topics.layout', 'board.topics.typography',
    'board.post.markdown', 'board.attachments.pdf', 'board.attachments.model'
));

INSERT INTO plugin_catalog (id, version, name, description, slot, stylesheet_path, is_approved) VALUES
    ('simple-markdown', '1.0.0', 'Simple Markdown', 'Bold, italic, lists, links and a writing preview for posts.', 'board.post.markdown', '/plugins/simple-markdown.css', FALSE),
    ('pdf-preview', '1.0.0', 'PDF preview', 'Attach and preview PDF documents on public posts.', 'board.attachments.pdf', '/plugins/pdf-preview.css', FALSE),
    ('model-preview', '1.0.0', '3D model preview', 'Attach and rotate self-contained GLB models on public posts.', 'board.attachments.model', '/plugins/model-preview.css', FALSE);
