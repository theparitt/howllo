# Howllo Server Architecture

## Overview

Howllo Server is a feedback management backend built with Rust, Actix-web, and PostgreSQL.

## Layers

```
HTTP Handler  →  Service  →  Repository  →  PostgreSQL
  (parse)        (logic)     (SQL only)      (storage)
```

### Handlers (`src/*/api.rs`)
- Parse HTTP request/path/body
- Extract auth context
- Call service
- Return HTTP response

No business SQL in handlers.

### Services (`src/services/`)
- Permission checks
- Business validation
- Workflow logic
- Transaction management
- Audit logging
- Notifications
- Webhook dispatching
- Realtime events

### Repositories (`src/repositories/`)
- SQL queries only
- Return domain DTOs
- No business logic

## Key Modules

| Module | Purpose |
|--------|---------|
| `src/domain/` | Role, Permission, Status enums and validation |
| `src/auth/` | JWT auth, API token auth, permission checks |
| `src/audit/` | Audit log service and repository |
| `src/realtime/` | WebSocket realtime events |
| `src/webhooks/` | Outbound webhook delivery with HMAC |
| `src/notifications/` | In-app follower notifications |
| `src/search/` | Full-text search for posts |
| `src/ai/` | AI suggestion system (advisory only) |

## Database

PostgreSQL with `sqlx` for compile-time checked queries.

Migrations in `db/migrations/` — numbered sequentially, idempotent (`IF NOT EXISTS`).

## Configuration

All configuration via environment variables. See `.env.example` and `docs/configuration.md`.

## Testing

- Actix-web integration tests (`cargo test`)
- In-memory rate limiting
- Shared test database with per-test reset
- 80+ tests covering all phases
