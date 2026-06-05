# Database

Howllo Server uses PostgreSQL with `sqlx` for compile-time checked queries.

## Migrations

Migrations are in `db/migrations/` and run sequentially:

```bash
DATABASE_URL=postgres://postgres:password@localhost:5432/howllo sqlx migrate run --source db/migrations
```

## Migration Files

| File | Contents |
|------|----------|
| `20240101000000_init.sql` | Core schema: tenants, users, boards, posts, comments, votes, tags, audit_logs, ai_suggestions |
| `20260602100000_*.sql` | Comment types, post follows, duplicates, status reason, tags |
| `20260602160000_*.sql` | Notifications, moderation notes, webhooks, API tokens |
| `20260603090000_*.sql` | Domain constraints, status/role validation |
| `20260603113000_*.sql` | Audit logs (idempotent recreation) |
| `20260603143000_*.sql` | Security, search index, AI, memberships |
| `20260603150000_*.sql` | API token expiry and last_used_at |
| `20260603160000_*.sql` | PostgreSQL full-text search (tsvector, GIN index, trigger) |
| `20260603161000_*.sql` | Webhook retry fields |

## Schema

Key tables: `tenants`, `boards`, `posts`, `comments`, `post_votes`, `post_follows`, `tags`, `post_tags`, `post_status_history`, `memberships`, `notifications`, `moderation_notes`, `audit_logs`, `webhook_endpoints`, `webhook_events`, `webhook_deliveries`, `api_tokens`, `ai_suggestions`

## SQLx Offline Mode

The `.sqlx/` directory contains pre-computed query metadata for CI builds without a database connection:

```bash
cargo sqlx prepare -- --all-targets
cargo sqlx prepare --check -- --all-targets
```

## Backup / Restore

See [backup-restore.md](backup-restore.md).
