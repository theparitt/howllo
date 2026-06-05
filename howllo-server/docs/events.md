# Events

Howllo Server emits events through three channels:

## Event Types

### `post.status_changed`

Fired when an admin changes a post's status.

- **Webhook**: Delivered to active tenant webhooks
- **Notification**: Pushed to followers who have `notify_on_status_change = true`
- **Realtime**: Broadcast via WebSocket to connected clients
- **Audit**: Recorded in `audit_logs` with old/new status values

### `comment.official_response`

Fired when a comment is marked as the official team response.

- **Webhook**: Delivered to active tenant webhooks
- **Notification**: Pushed to followers who have `notify_on_official_response = true`

## Webhook Event Format

```json
{
    "event_type": "post.status_changed",
    "payload": {
        "post_id": "uuid",
        "old_status": "planned",
        "new_status": "in_progress",
        "reason": "Starting work on this"
    }
}
```

### Webhook Headers

| Header | Description |
|--------|-------------|
| `x-howllo-event-id` | Unique event UUID |
| `x-howllo-timestamp` | Unix timestamp of delivery |
| `x-howllo-signature` | HMAC SHA-256 signature (`sha256=...`) |

## Notification Format

In-app notifications are stored in the `notifications` table:

| Field | Description |
|-------|-------------|
| `event_type` | `status_changed` or `official_response` |
| `title` | Human-readable title |
| `body` | Human-readable body |
| `is_read` | Whether the user has read it |
| `post_id` | Related post UUID |

## Realtime (WebSocket)

Connect via WebSocket to receive live events. Events are broadcast per tenant.

## Audit Actions

Every admin/moderator mutation is recorded. See `audit_logs` table. Key action constants:

`board_created`, `board_updated`, `post_status_changed`, `post_hidden`, `post_restored`, `post_soft_deleted`, `post_locked`, `comment_hidden`, `comment_marked_official`, `tag_created`, `tag_updated`, `tag_deleted`, `tag_attached`, `tag_detached`, `duplicate_marked`, `duplicate_removed`, `webhook_created`, `webhook_deactivated`, `api_token_created`, `api_token_revoked`, `member_role_changed`, `member_removed`, `export_posts_json`, `export_posts_csv`, `ai_suggestion_reviewed`
