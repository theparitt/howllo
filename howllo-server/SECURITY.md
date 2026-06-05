# Security

## Authentication

Howllo supports two authentication methods:

1. **RooIAM JWT** — User authentication via JWT tokens issued by RooIAM. Required for all write operations.
2. **API Tokens** — Read-only tokens created per-tenant. Use `Bearer howllo_<prefix>_<uuid>` header.

## Authorization

See [docs/permissions.md](docs/permissions.md) for the full role/permission model.

- **Owner** — Full access
- **Admin** — Manage boards, tags, webhooks, API tokens, members
- **Moderator** — Hide posts/comments, mark duplicates, lock posts
- **Member** — Create posts, vote, comment, follow

## Rate Limiting

Public write endpoints (POST/PATCH/DELETE on `/api/boards`, `/api/posts`) are rate-limited. Configure via:
- `RATE_LIMIT_ENABLED=true`
- `PUBLIC_WRITE_RATE_LIMIT=50` (requests per window)

## Input Validation

- Post body: max length via `MAX_POST_BODY_CHARS`
- Comment body: max length via `MAX_COMMENT_BODY_CHARS`
- All inputs validated server-side

## Webhook Security

- Webhook payloads are signed with HMAC SHA-256 (`x-howllo-signature`)
- Delivery includes `x-howllo-event-id` and `x-howllo-timestamp`
- Webhook secrets are never returned in list responses
- Timeout configurable via `HOWLLO_WEBHOOK_TIMEOUT_MS`

## API Token Security

- Tokens stored as SHA-256 hashes (not plaintext)
- Tokens have tenant scope and permission scopes
- Tokens support `expires_at` and auto-update `last_used_at`
- Revoked tokens are rejected immediately

## CORS

Configure allowed origins via `ALLOWED_ORIGINS` env var. Never use wildcard in production.

## Reporting

Report security issues to security@howllo.app
