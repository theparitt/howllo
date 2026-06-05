UPDATE posts
SET status = 'under_review'
WHERE status = 'under-review';

UPDATE posts
SET status = 'in_progress'
WHERE status = 'in-progress';

UPDATE post_status_history
SET old_status = 'under_review'
WHERE old_status = 'under-review';

UPDATE post_status_history
SET old_status = 'in_progress'
WHERE old_status = 'in-progress';

UPDATE post_status_history
SET new_status = 'under_review'
WHERE new_status = 'under-review';

UPDATE post_status_history
SET new_status = 'in_progress'
WHERE new_status = 'in-progress';

ALTER TABLE posts
ALTER COLUMN status SET DEFAULT 'under_review';

ALTER TABLE memberships
DROP CONSTRAINT IF EXISTS memberships_role_check;

ALTER TABLE posts
DROP CONSTRAINT IF EXISTS posts_status_check;

ALTER TABLE post_status_history
DROP CONSTRAINT IF EXISTS post_status_history_old_status_check;

ALTER TABLE post_status_history
DROP CONSTRAINT IF EXISTS post_status_history_new_status_check;

ALTER TABLE memberships
ADD CONSTRAINT memberships_role_check
CHECK (role IN ('owner', 'admin', 'moderator', 'member'));

ALTER TABLE posts
ADD CONSTRAINT posts_status_check
CHECK (status IN ('under_review', 'planned', 'in_progress', 'done', 'declined'));

ALTER TABLE post_status_history
ADD CONSTRAINT post_status_history_old_status_check
CHECK (
    old_status IS NULL OR
    old_status IN ('under_review', 'planned', 'in_progress', 'done', 'declined')
);

ALTER TABLE post_status_history
ADD CONSTRAINT post_status_history_new_status_check
CHECK (new_status IN ('under_review', 'planned', 'in_progress', 'done', 'declined'));
