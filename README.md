# Howllo

[![Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![GitHub](https://img.shields.io/badge/GitHub-theparitt%2Fhowllo-181717?logo=github)](https://github.com/theparitt/howllo)

Howllo is open source under the [Apache License 2.0](LICENSE). Contributions are welcome on [GitHub](https://github.com/theparitt/howllo).

Multi-tenant feedback and roadmap platform. Customers submit and
vote on feedback; product teams triage, set roadmap status, and respond.

Howllo is identity-provider agnostic. RooIAM is supported as an optional integration, but is not required. A fresh installation supports local accounts. You can also use Google, Microsoft Entra ID, Keycloak, Authentik or any compatible OpenID Connect provider through the generic OIDC adapter. See [authentication setup](docs/auth/README.md).

## Monorepo layout

```text
howllo/
  howllo-server/      # Rust + Actix + SQLx + PostgreSQL + MinIO   :7700
  howllo-admin/       # Vite + React + TypeScript (platform operator console) :7701
  howllo-app/         # Next.js (tenant management, RooIAM login)              :7702
  howllo-web/         # Next.js (public boards, local accounts)                 :7703
  howllo-landing/     # Marketing site                             :5114
  howllo-docs/        # Product guides and user tutorials           :5116
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

Tenant management is **Howllo App** at port 7702 `/app/{workspace}`. Customers use **Howllo Web** at port 7703 `/{workspace}`. They share the server's workspace and board records but have separate login experiences; `howllo-admin` remains the platform operator console. See [product surfaces](docs/product-surfaces.md).

## Ports (canonical — `shared/config/src/ports.ts`)

| Port | Service          |
| ---- | ---------------- |
| 7700 | howllo-server    |
| 7701 | howllo-admin     |
| 7702 | howllo-app       |
| 7703 | howllo-web       |
| 7700/ws | board websocket  |
| 5114 | howllo-landing   |
| 5115 | howllo-widget (reserved, later) |
| 5116 | howllo-docs      |

## Local development

```bash
# Optional local backing services (skip when using PostgreSQL and MinIO on 192.168.0.147)
docker compose up -d

# Backend (port 7700), from the repository root
cp howllo-server/.env.example howllo-server/.env
cd howllo-server
sqlx migrate run --source db/migrations
cargo run

# In a second terminal, from the repository root
npm install
npm run dev:app      # :7702
npm run dev:web      # :7703
npm run dev:admin    # :7701
npm run dev:landing  # :5114
npm run dev:docs     # :5116
```

For external backing services, set `HOWLLO_DATABASE_URL` and the
`HOWLLO_MINIO_*` variables in `howllo-server/.env`. Set
`HOWLLO_STORAGE_PUBLIC_BASE_URL` to the browser-accessible bucket URL, for
example `http://192.168.0.147:9000/howllo`. Saved storage settings in
PostgreSQL override `.env` values. Keep `DATABASE_URL` in sync when running the
`sqlx` CLI.

Copy `howllo-app/.env.example` to `howllo-app/.env.local`,
`howllo-web/.env.example` to `howllo-web/.env.local`, and
`howllo-admin/.env.example` to `howllo-admin/.env.local` before changing the
public API URL. The API URL in those files must be reachable from users'
browsers. The backend CORS list must include both frontend origins.

Howllo Web reads login options from `GET /api/auth/providers`. Keep
`HOWLLO_AUTH_LOCAL_ENABLED=true` on the server for its end-user accounts.
Howllo App uses the RooIAM widget and callback on port 7702. To add an
OIDC provider, set `HOWLLO_PUBLIC_API_URL`, `HOWLLO_WEB_ORIGIN` and
`HOWLLO_OIDC_PROVIDERS` as described in [docs/auth/oidc.md](docs/auth/oidc.md).
Set `NEXT_PUBLIC_ROOIAM_WIDGET_*` in `howllo-app/.env.local` and
`HOWLLO_ROOIAM_*` on the server; see [RooIAM setup](docs/auth/rooiam.md).

## Production routing

```text
api.howllo.dev       -> howllo-server through Cloudflare Tunnel
app.howllo.dev       -> howllo-app (tenant management)
feedback.howllo.dev  -> howllo-web (public boards)
admin.howllo.dev     -> howllo-admin (platform operators)
howllo.dev           -> howllo-landing
www.howllo.dev       -> howllo-landing
docs.howllo.dev      -> howllo-docs
```

## Deploy frontends to Cloudflare Workers

Install dependencies with `npm ci`, copy `.env.production.example` to
`.env.production`, and fill in the public HTTPS API, App, and Web origins plus
the RooIAM widget workspace and client IDs. The root deployment script reads
`.env.production`; shell environment variables take precedence. The file is
gitignored. Keep the API URL reachable from both browsers and Cloudflare
Workers; `192.168.x.x` and `localhost` are not public origins.

Run `npm run deploy:check` to build all frontends and perform Wrangler
dry runs. Run `npm run deploy` to publish separate Workers for `howllo-app`,
`howllo-web`, `howllo-admin`, `howllo-landing`, and `howllo-docs`. Authenticate Wrangler with
`npx wrangler login` (or a Cloudflare API token in CI). Wrangler binds the
production domains above as Worker custom domains. Register
`https://app.howllo.dev/auth/callback` (or your chosen App domain) with RooIAM,
and configure the backend's public origin/CORS for App, Web, and Admin. The
`api.howllo.dev` DNS record is a proxied CNAME to the `howllo-api` Cloudflare
Tunnel, whose connector runs on the API host. On the current host,
`howllo-server.service` and `howllo-api-tunnel.service` keep the API and tunnel
running. The tunnel forwards `/howllo/*` to the private MinIO bucket and all
other `api.howllo.dev` paths to the Rust API. The backend `howllo-server`,
PostgreSQL, and MinIO remain on your own infrastructure; this command does not
deploy them.

Platform administrators set workspace rate defaults and ceilings in **Admin → Platform settings → Limits**. They can also set a hard storage cap for each workspace. Workspace owners and admins use **App → Security** to set their own limits; empty fields inherit the platform default. Defaults allow 6 posts and 60 comments per member per hour, with daily limits of 30 and 300. Board bursts pause new writes for five minutes. The default workspace storage quota is 500 MB. Uploads belong to a workspace; existing referenced logos, board icons, and post attachments are measured when its policy is first viewed or an upload is attempted. IP and country rules apply to posts, comments, and uploads. Country rules require Cloudflare's `CF-IPCountry` header.

The two Next.js Workers use OpenNext. Their `NEXT_PUBLIC_*` values are baked
into browser bundles at build time, so rebuild after changing domains. The
Admin, Landing, and Guides Workers serve static assets. Read the product
tutorials at [docs.howllo.dev](https://docs.howllo.dev), or run `npm run
dev:docs` while developing locally.

See [docs/architecture.md](docs/architecture.md) for the full architecture,
[the production roadmap](docs/roadmap.md) for release gates, and
[docs/internal/](docs/internal/) for deeper design notes. The
[MVP verification record](docs/MVP_READINESS.md) is a historical snapshot.
