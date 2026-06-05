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
| howllo-server  | Rust + Actix + SQLx + Postgres | 5110 | source of truth / API     |
| howllo-admin   | Vite + React + TypeScript      | 5111 | moderation / tenant admin |
| howllo-web     | Next.js + TypeScript           | 5112 | public feedback boards    |
| howllo-landing | Static / Next                  | 5114 | marketing                 |
| howllo-docs    | Docs site                      | 5116 | documentation             |

Why split: admin needs no SEO; public boards and landing do; docs stay static.

## Shared code (`shared/`)

`shared/types` and `shared/api-client` keep the frontends in sync with the
server contract. `shared/config` is the single source of truth for ports and
route builders. `shared/ui` holds shared React components.

## Identity model

- **Rooiam** = identity provider (who the user is).
- **Howllo** = authorization (what the user can do in a tenant).

Every tenant-owned entity carries `tenant_id`. Tenant scoping is enforced
server-side and never trusted from the frontend.

## Board hierarchy

`tenant.slug + board.slug` uniquely identifies a board. Internal references use
UUID `id`; slugs are display/URL only.
