# Security

## Authentication

Howllo supports two auth methods:

1. **RooIAM JWT** — User authentication for write operations
2. **API Tokens** — Scoped read tokens for integrations (prefixed `howllo_`)

## Authorization

Role-based access control with 4 roles: owner, admin, moderator, member. See [permissions.md](permissions.md).

## Tenant Isolation

Every query is tenant-scoped. Cross-tenant access returns `404 Not Found`. No data leaks between tenants.

## Input Validation

- Post title max length: 255 characters (fixed)
- Post body max length: `HOWLLO_MAX_POST_BODY_CHARS` (default 10000)
- Comment body max length: `HOWLLO_MAX_COMMENT_BODY_CHARS` (default 4000)

## Rate Limiting

- PostgreSQL-backed per-IP limit for public write endpoints, shared across API instances
- Configurable via `HOWLLO_RATE_LIMIT_ENABLED` and `HOWLLO_PUBLIC_WRITE_RATE_LIMIT`
- Client IP headers are accepted only from networks in `HOWLLO_TRUSTED_PROXY_CIDRS`

## CORS

Configure allowed origins via `HOWLLO_ALLOWED_ORIGINS`. Never use wildcard in production.

## Webhook Security

- Payloads signed with HMAC SHA-256
- Secrets never returned in list responses
- Deactivated webhooks do not receive deliveries
- Timeout configurable

## API Token Security

- Stored as SHA-256 hashes
- Tenant-scoped
- Support expiry and revocation
- `last_used_at` tracked

## Audit Log

Every admin/moderator mutation is recorded with actor, action, old/new values, and request ID.

## Reporting

Report security issues to security@howllo.app
