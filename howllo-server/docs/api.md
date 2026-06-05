# API Reference

Full OpenAPI specification: [openapi.yaml](../openapi.yaml) (732 lines, 50+ endpoints documented).

## Authentication

Requests use `Authorization: Bearer <token>`. Two token types:

1. **JWT tokens** (from RooIAM) — for user-authenticated requests. Required for write operations.
2. **API tokens** (prefixed `howllo_`) — for integration read access. Scoped per endpoint group.

## Common Patterns

### Request ID

Every response includes `x-request-id` header for tracing.

### Error Response

```json
{
    "error": {
        "code": "validation_error",
        "message": "title is required",
        "request_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
    }
}
```

Error codes: `unauthorized`, `forbidden`, `not_found`, `bad_request`, `validation_error`, `too_many_requests`, `internal_server_error`

HTTP status codes: `200 OK`, `201 Created`, `400 Bad Request`, `401 Unauthorized`, `403 Forbidden`, `404 Not Found`, `422 Unprocessable Entity`, `429 Too Many Requests`, `500 Internal Server Error`

### Paginated Response

```json
{
    "items": [],
    "page": 1,
    "per_page": 20,
    "total": 100,
    "has_next": true
}
```

Query parameters: `page` (default 1), `per_page` (default 20, max 100)

### Tenant Scoping

Most endpoints require `tenant_slug` query parameter. All data is tenant-scoped. Cross-tenant access returns `404 Not Found`.

## Endpoint Groups

| Group | Base Path | Auth |
|-------|-----------|------|
| Health | `/api/health`, `/api/ready` | None |
| Boards | `/api/boards`, `/api/admin/boards` | Public/Admin |
| Posts | `/api/boards/{slug}/posts`, `/api/posts/{id}` | Public/Member+ |
| Comments | `/api/posts/{id}/comments` | Public/Member+ |
| Votes | `/api/posts/{id}/vote` | Member+ |
| Follows | `/api/posts/{id}/follow` | Member+ |
| Tags | `/api/tags`, `/api/admin/tags` | Public/Admin |
| Moderation | `/api/admin/posts/{id}/status`, `/visibility`, `/soft-delete`, `/restore`, `/duplicate`, `/lock` | Admin/Moderator |
| Queue | `/api/admin/moderation/queue` | Moderator+ |
| Members | `/api/admin/members` | Admin |
| Roadmap | `/api/roadmap`, `/api/roadmap/grouped` | Public |
| Webhooks | `/api/admin/webhooks` | Admin |
| API Tokens | `/api/admin/api-tokens` | Admin |
| Notifications | `/api/notifications` | Authenticated |
| Search | `/api/search/posts` | Public |
| AI | `/api/admin/ai/posts/{id}/*` | Admin |
| Audit | `/api/admin/audit-logs` | Admin |
| Export | `/api/admin/export/posts.{json,csv}` | Admin |

## Status Values

| Status | Description | Public Roadmap |
|--------|-------------|----------------|
| `under_review` | Pending team review | ❌ |
| `planned` | Approved for future work | ✅ |
| `in_progress` | Currently being worked on | ✅ |
| `done` | Completed | ✅ |
| `declined` | Won't implement | ❌ |
