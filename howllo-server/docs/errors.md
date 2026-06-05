# Errors

All API errors follow a consistent structure.

## Response Format

```json
{
    "error": {
        "code": "validation_error",
        "message": "title is required",
        "request_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
    }
}
```

Every error includes a `request_id` for traceability via the `x-request-id` response header.

## Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `unauthorized` | 401 | Missing or invalid authentication |
| `forbidden` | 403 | Valid auth but insufficient permissions |
| `not_found` | 404 | Resource does not exist or is hidden/deleted |
| `bad_request` | 400 | Malformed request |
| `validation_error` | 422 | Request data fails validation rules |
| `too_many_requests` | 429 | Rate limit exceeded |
| `internal_server_error` | 500 | Unexpected server error |

## Common Scenarios

### Cross-tenant access
Returns `404 Not Found` (not `403 Forbidden`) to avoid leaking tenant existence.

### Hidden/deleted content
Returns `404 Not Found` to non-admin/moderator users.

### Private board access
Anonymous users: `403 Forbidden`. Non-members: `403 Forbidden`.

### Invalid status transition
```json
{
    "error": {
        "code": "validation_error",
        "message": "cannot transition from declined to done",
        "request_id": "..."
    }
}
```

### Rate limited
```json
{
    "error": {
        "code": "too_many_requests",
        "message": "rate limit exceeded. try again later.",
        "request_id": "..."
    }
}
```

### Expired API token
Returns `401 Unauthorized` (same as missing auth to avoid leaking token validity).

### Invalid API token scope
Returns `403 Forbidden`.
