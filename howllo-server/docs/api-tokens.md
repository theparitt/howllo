# API Tokens

API tokens provide scoped read access for integrations.

## Create Token

`POST /api/admin/api-tokens`

```json
{
    "tenant_slug": "acme",
    "name": "Dashboard Integration",
    "scopes": ["posts:read", "roadmap:read"]
}
```

Response (token shown only once):
```json
{
    "id": "uuid",
    "name": "Dashboard Integration",
    "token": "howllo_abc123_def456...",
    "token_prefix": "abc123",
    "scopes": ["posts:read", "roadmap:read"]
}
```

## Using a Token

```bash
curl -H "Authorization: Bearer howllo_abc123_def456..." \
  "http://localhost:5110/api/posts/{id}?tenant_slug=acme"
```

## Scopes

| Scope | Access |
|-------|--------|
| `posts:read` | Read posts and boards |
| `comments:read` | Read comments |
| `roadmap:read` | Read roadmap |
| `webhooks:write` | Reserved |
| `admin:read` | Reserved |

Tokens without the required scope receive `403 Forbidden`.

## Token Lifecycle

- **Created** — Active immediately, raw token returned once
- **Used** — `last_used_at` updated on each successful request
- **Expired** — Rejected if `expires_at` is in the past
- **Revoked** — Rejected if `revoked_at` is set

`PATCH /api/admin/api-tokens/{token_id}/revoke` to revoke.

## Security

- Tokens stored as SHA-256 hashes (not plaintext)
- Tokens are tenant-scoped — cannot access other tenants
- Write operations require JWT authentication, not API tokens
- Create/revoke requires admin permission
