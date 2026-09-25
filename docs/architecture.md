# Architecture

This is the top-level overview. Deeper design notes live in
[internal/ARCHITECTURE.md](internal/ARCHITECTURE.md).

## Principle

Howllo is backend-centric. Business rules, tenant isolation, and authorization
live in `howllo-server`. Frontends are thin clients. AI is an optional assistive
layer, never on the correctness path.

## Apps

| App            | Stack                          | Port | Responsibility            |
| -------------- | ------------------------------ | ---- | ------------------------- |
| howllo-server  | Rust + Actix + SQLx + Postgres | 7700 | source of truth / API     |
| howllo-admin   | Vite + React + TypeScript      | 7701 | platform operations       |
| howllo-app     | Next.js + TypeScript           | 7702 | tenant board management   |
| howllo-web     | Next.js + TypeScript           | 7703 | public feedback boards    |
| howllo-landing | Static / Next                  | 5114 | marketing                 |
| howllo-docs    | Docs site                      | 5116 | documentation             |

Tenant managers sign in to Howllo App with RooIAM. Public Web uses Howllo local accounts and shows boards under `/{workspace}/boards/{board}`. Both frontends use the same API and records.

## Shared code (`shared/`)

`shared/types` and `shared/api-client` keep the frontends in sync with the
server contract. `shared/config` is the single source of truth for ports and
route builders. `shared/ui` holds shared React components.

## Identity model

- **RooIAM** establishes tenant manager identity in Howllo App. **Howllo local accounts** establish public participant identity in Howllo Web. The server can also support configured OIDC providers in other deployments.
- **Howllo** maps each `(provider_id, subject)` to a stable local user ID and owns workspace authorization.

Every tenant-owned entity carries `tenant_id`. Tenant scoping is enforced
server-side and never trusted from the frontend.

## Board hierarchy

`tenant.slug + board.slug` uniquely identifies a board. Internal references use
UUID `id`; slugs are display/URL only.
