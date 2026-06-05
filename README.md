# Howllo

Multi-tenant feedback and roadmap platform (open-core). Customers submit and
vote on feedback; product teams triage, set roadmap status, and respond.

## Monorepo layout

```text
howllo/
  howllo-server/      # Rust + Actix + SQLx + PostgreSQL + MinIO   :5110
  howllo-admin/       # Vite + React + TypeScript (admin console)  :5111
  howllo-web/         # Next.js (public feedback boards)           :5112
  howllo-landing/     # Marketing site                             :5114
  howllo-docs/        # Documentation site                         :5116
  shared/             # code shared across the frontends (npm @howllo/* packages)
    api-client/       # Typed client over the server REST API
    types/            # Shared TS types mirroring server DTOs
    ui/               # Shared React components (StatusBadge, Tag, …)
    config/           # Canonical ports, env, route builders
    eslint-config/    # Shared ESLint base
    tsconfig/         # Shared tsconfig bases
  docs/               # architecture / api / permissions / roadmap
```

## Board identity

Board identity is hierarchical and unique by `tenant.slug + board.slug`:

- A **tenant** (company/workspace) has a unique `slug` — typically the company
  name plus a short random suffix (5–6 chars) to keep it globally unique.
- A **board** has a human `name` (display) and a `slug` (URL). Boards are unique
  per tenant: `UNIQUE(tenant_id, slug)`.
- Both tenants and boards also carry an internal UUID `id`. Slugs are for
  display/URLs; system references use the `id`.

One admin can control multiple boards across multiple tenants.

## Ports (canonical — `shared/config/src/ports.ts`)

| Port | Service          |
| ---- | ---------------- |
| 5110 | howllo-server    |
| 5111 | howllo-admin     |
| 5112 | howllo-web       |
| 5113 | board websocket  |
| 5114 | howllo-landing   |
| 5115 | howllo-widget (reserved, later) |
| 5116 | howllo-docs      |

## Local development

```bash
# Backing services (Postgres :5433, MinIO :9000/:9001)
docker compose up -d

# Backend (port 5110)
cd howllo-server && cargo run

# Frontends
npm run dev:web      # :5112
npm run dev:admin    # :5111
npm run dev:landing  # :5114
npm run dev:docs     # :5116
```

## Production routing

```text
api.howllo.com       -> howllo-server
app.howllo.com       -> howllo-admin
feedback.howllo.com  -> howllo-web   (later: feedback.customer.com)
howllo.com           -> howllo-landing
docs.howllo.com      -> howllo-docs
```

See [docs/architecture.md](docs/architecture.md) for the full architecture and
[docs/internal/](docs/internal/) for deeper design notes.
