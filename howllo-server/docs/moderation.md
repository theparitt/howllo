# Moderation

## Moderation Queue

`GET /api/admin/moderation/queue?tenant_slug=acme`

Returns posts that need attention:
- Posts with `is_hidden = true`
- Posts with `status = 'under_review'`

Supports board filtering: `?board_slug=features`

## Post Moderation

### Change Status

`PATCH /api/admin/posts/{post_id}/status`

Valid transitions:
```
under_review → planned | declined
planned      → in_progress | declined  
in_progress  → done | planned
done         → in_progress
declined     → under_review
```

Declined status requires a reason.

### Hide/Restore

`PATCH /api/admin/posts/{post_id}/visibility`

Toggles `is_hidden`. Hidden posts are invisible to non-moderators.

### Soft Delete

`PATCH /api/admin/posts/{post_id}/soft-delete`

Sets `deleted_at` and hides the post. Can be restored.

### Restore

`PATCH /api/admin/posts/{post_id}/restore`

Clears `deleted_at` and `is_hidden`.

### Lock

`PATCH /api/admin/posts/{post_id}/lock`

Locked posts reject new comments and votes. Follows are still allowed.

### Duplicate

`PATCH /api/admin/posts/{post_id}/duplicate`

Marks a post as duplicate of another. Rules:
- Cannot duplicate itself
- Canonical must be in same tenant
- Canonical must not be hidden
- Canonical must not be deleted
- Canonical must not itself be a duplicate

## Comment Moderation

### Official Response

`PATCH /api/admin/comments/{comment_id}/official`

Marks a comment as the official team response. Triggers follower notifications.

### Hide Comment

`PATCH /api/admin/comments/{comment_id}/visibility`

Hides a comment from public view. Moderators can see hidden comments via `?include_hidden=true`.

## Moderation Notes

`POST /api/admin/posts/{post_id}/notes`
`GET /api/admin/posts/{post_id}/notes`

Internal notes visible only to admins and moderators. Not exposed to members or public.

## Audit Log

Every moderation action is recorded in `audit_logs` with:
- `actor_user_id` — who performed the action
- `action` — what was done
- `old_value` / `new_value` — before/after state
- `request_id` — for traceability
