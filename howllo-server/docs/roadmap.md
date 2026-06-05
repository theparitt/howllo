# Roadmap

The roadmap is a public-facing view of feedback progress.

## Endpoints

### Standard Roadmap

`GET /api/roadmap?tenant_slug=acme`

Returns flat list of posts with status `planned`, `in_progress`, or `done`.

Filters:
- `?status=planned` — filter by status
- `?tag=ux` — filter by tag
- `?board_slug=features` — filter by board

### Grouped Roadmap

`GET /api/roadmap/grouped?tenant_slug=acme`

Returns:
```json
{
    "planned": [...],
    "in_progress": [...],
    "done": [...]
}
```

## Visibility Rules

The roadmap automatically excludes:
- Hidden posts (`is_hidden = true`)
- Deleted posts (`deleted_at IS NOT NULL`)
- Duplicate posts (`duplicate_of_post_id IS NOT NULL`)
- Declined posts
- Under review posts
- Private board posts (for anonymous users)

Private board posts are visible to authorized members of that board's tenant.

## Sorting

Posts within each status group are sorted by status priority then creation date:
1. `in_progress` first
2. `planned` second
3. `done` last

## Security

The roadmap never leaks:
- Internal board content to non-members
- Hidden or deleted posts
- Cross-tenant data
