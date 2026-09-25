# Howllo Technical Structure

## 1. Repository Layout

Recommended repositories:

- `howllo-server`
- `howllo-web`
- `howllo-admin`
- `howllo-landing`
- `howllo-docs`

This structure keeps responsibilities clear and prevents product, admin, marketing, and documentation concerns from being mixed together.


## 2. Naming Decision

### Recommended name

Use:

- `howllo-web`

Do not use:

- `howllo-app`
- `howllo-frontend`

### Why `howllo-web` is better

`howllo-web` is the cleanest name for the main user-facing product UI.

It is better than `howllo-app` because `app` is vague and can refer to the whole platform, mobile clients, admin panels, or any executable product surface.

It is better than `howllo-frontend` because `frontend` sounds like an implementation detail rather than a product surface.

Recommended meaning:

- `howllo-web` = the public/product-facing board UI
- `howllo-admin` = internal admin and moderation UI
- `howllo-landing` = marketing site
- `howllo-docs` = documentation site


## 3. Stack Decision

### Final recommendation

Use:

- Rust + Actix Web for backend
- PostgreSQL for primary database
- SQLx for database access
- Next.js for `howllo-web`
- board admin / moderator surface on `7701`
- board realtime websocket endpoint on `7700/ws`
- Next.js for `howllo-landing`
- static docs site for `howllo-docs`

### Why this stack is recommended

This matches the current direction of the codebase:

- `howllo-web` is the main product UI and now lives on Next.js
- `howllo-server` remains the source of truth for business rules
- the admin/moderator surface is still a separate concern and keeps its own reserved port
- board realtime can be introduced as a dedicated websocket surface without colliding with admin

Local port rule:

- `7700` = API and board websocket at `/ws`
- `7701` = board admin / moderator
- `7702` = public feedback boards
- `5115` = docs / help surface


## 4. Why Vite for `howllo-web` and `howllo-admin`

Vite is recommended for the main product UIs because:

- fast development cycle
- simpler build system
- less framework overhead
- easier separation from backend API
- ideal for SPA or API-driven application UIs
- avoids unnecessary server-rendering complexity

The public Howllo board does not need deep SSR at first.
The important behavior is:

- listing posts
- creating posts
- comments
- voting
- filtering
- roadmap views
- admin controls

These are all very comfortable in a Vite-based frontend.

### Why not Next.js for everything

Using Next.js for all surfaces would add complexity where it does not create enough value.

Examples of things that are not worth forcing into Next.js for the app/admin surfaces:

- route handlers duplicated outside Actix
- mixed backend/frontend responsibilities
- more deployment complexity
- more mental overhead
- unnecessary server-side rendering for dashboard-style UIs

Since you already want Actix as the real backend, it is cleaner to let Actix be the backend and let Vite apps consume its APIs.


## 5. Surface Responsibilities

## `howllo-server`

Primary backend service.

Responsibilities:

- REST API
- authentication/session integration
- tenant management
- board logic
- post/comment/vote/status logic
- moderation rules
- notifications later
- AI integration layer later
- admin authorization
- rate limiting
- audit-related data handling

This is the source of truth for business logic.


## `howllo-web`

Main public product UI.

Responsibilities:

- public board browsing
- post detail pages
- create signal
- comment
- vote
- filter/sort
- roadmap view
- user profile basics later
- sign-in flow handoff to Rooiam

This should be the user-facing application surface.


## `howllo-admin`

Internal admin/moderation UI.

Responsibilities:

- tenant settings
- board settings
- moderation actions
- status updates
- official responses
- tag management
- duplicate handling later
- analytics later
- AI-assisted admin workflows later

This should stay separate from `howllo-web` so admin complexity does not pollute the public UX.


## `howllo-landing`

Marketing site.

Responsibilities:

- homepage
- feature pages
- pricing page
- open source page
- changelog summary page if desired
- signup CTA
- integration with docs
- blog later if needed

This should optimize for:

- SEO
- content
- conversion
- positioning

This is why Next.js fits here.


## `howllo-docs`

Documentation site.

Responsibilities:

- installation guide
- self-hosting guide
- API docs
- configuration docs
- auth integration docs
- developer docs
- future open-core/commercial boundary explanation

This can remain separate so docs are versionable and not mixed with the marketing site.

Local development convention:

- serve `howllo-docs` on `5115`
- treat it as the board help and product documentation surface


## 6. Architecture Style

Recommended architecture style:

- backend-centric business logic
- frontend as thin client
- explicit API boundaries
- clear tenant scoping
- no duplicated business rules in multiple frontends

### Guiding principle

All important logic should live in `howllo-server`.

That includes:

- validation
- permissions
- workflow state transitions
- tenant isolation
- moderation rules
- status changes
- duplicate merge rules later

The frontends should focus on presentation and user interaction, not owning business logic.


## 7. Backend Technical Direction

## Language and framework

- Rust
- Actix Web

## Data access

- SQLx
- PostgreSQL

## Suggested internal modules

Suggested server structure:

- `config`
- `startup`
- `http`
- `routes`
- `handlers`
- `auth`
- `tenancy`
- `boards`
- `posts`
- `comments`
- `votes`
- `tags`
- `statuses`
- `moderation`
- `users`
- `memberships`
- `notifications`
- `search`
- `ai`
- `db`
- `errors`
- `models`
- `dto`

### Suggested responsibility split

#### `routes`
Route registration only.

#### `handlers`
HTTP layer only.
Parse request, call service/domain layer, return response.

#### `boards`, `posts`, `comments`, etc.
Actual domain logic.

#### `db`
Queries and database access layer.

#### `dto`
Request/response structures.

#### `models`
Domain-facing types and persistence-facing types if needed.

#### `errors`
Shared error types and error mapping.

#### `ai`
Optional, isolated AI-related services and abstractions.

### Guiding rule

Avoid putting all business logic directly in handlers.
Handlers should stay thin.


## 8. Frontend Technical Direction

## `howllo-web`

Recommended approach:

- Vite
- React
- TypeScript
- TanStack Query
- React Router
- simple component library
- API client layer separated from components

Why React and not Next.js here:

- simpler
- faster to iterate
- no SSR dependency
- clean separation from Actix backend

## `howllo-admin`

Use the same stack as `howllo-web`:

- Vite
- React
- TypeScript
- TanStack Query
- React Router

This keeps developer experience consistent and avoids duplicated architectural complexity.

### UI separation rule

`howllo-web` and `howllo-admin` should not be one frontend with mode switches.
They should be separate frontends.
This keeps each surface cleaner and easier to reason about.


## 9. Auth Direction

Use Rooiam as the auth provider.

Recommended model:

- Howllo uses Rooiam for sign-in
- Howllo stores local user record mapped to external identity
- tenant membership and permissions are managed inside Howllo

### Why this is the right split

Rooiam should remain the identity source.
Howllo should remain the product authorization source.

That means:

- Rooiam answers: who is this user
- Howllo answers: what can this user do in this tenant

This is the cleanest architecture.


## 10. Multi-Tenant Direction

Recommended decision:

- multi-tenant in core backend from day one

### Rules

Every major tenant-owned record should include `tenant_id`.

Examples:

- boards
- posts
- comments if needed indirectly or directly
- tags
- memberships
- statuses
- notification settings
- admin settings

### Why this matters

This allows:

- Rooiam tenant
- AraiHub tenant
- Seavanna tenant
- future external tenant

without needing a later architectural rewrite.

### Scope rule

Architecturally multi-tenant.
Operationally simple at first.

Do not add unnecessary tenant complexity in V1.


## 11. Suggested URL Model

### Public landing

- `howllo.com`

### App surfaces

- `app.howllo.com` for `howllo-web`
- `admin.howllo.com` for `howllo-admin`
- `docs.howllo.com` for `howllo-docs`

Optional:

- `api.howllo.com` for backend

### Alternative

You may also keep API behind:

- `app.howllo.com/api`
- or direct service-origin routing behind reverse proxy

But conceptually it is cleaner to think of:

- landing
- app
- admin
- docs
- api

as separate surfaces.


## 12. Suggested Deployment Model

### Recommended services

- PostgreSQL
- `howllo-server`
- `howllo-web`
- `howllo-admin`
- `howllo-landing`
- `howllo-docs`

### Reverse proxy

Use reverse proxy to route domains/subdomains to the correct service.

### Example routing

- `howllo.com` -> `howllo-landing`
- `app.howllo.com` -> `howllo-web`
- `admin.howllo.com` -> `howllo-admin`
- `docs.howllo.com` -> `howllo-docs`
- `api.howllo.com` -> `howllo-server`

This is easy to reason about and easy to operate later.


## 13. Internal Domain Model Direction

Suggested first domain entities:

- tenants
- users
- memberships
- boards
- posts
- comments
- votes
- tags
- post_tags
- post_status_history

Optional later:

- duplicate_links
- notifications
- official_responses
- audit_entries
- saved_filters
- board_settings
- tenant_settings


## 14. API Direction

Recommended first API style:

- REST-first
- JSON
- explicit tenant-scoped routes where appropriate

Example route groups:

- `/auth/*`
- `/tenants/*`
- `/boards/*`
- `/posts/*`
- `/comments/*`
- `/votes/*`
- `/tags/*`
- `/admin/*`

### Why REST-first

REST is enough for V1 and easier to debug.
Do not introduce GraphQL or overly abstract APIs early.


## 15. AI / LLM Technical Direction

AI should be isolated from core CRUD logic.

Recommended design:

- core platform works without AI
- AI services are optional modules
- LLM outputs are assistive, not authoritative

### Suggested AI module responsibilities later

- duplicate suggestion
- related post suggestion
- discussion summarization
- tag suggestion
- board classification assistance
- moderation assistance

### Suggested abstraction approach

Use interfaces/traits like:

- `EmbeddingProvider`
- `CompletionProvider`
- `ModerationAssistProvider`

This keeps the system model-agnostic and easier to evolve.


## 16. Suggested Build Order

### Phase 1

Foundation.

Build:

- `howllo-server`
- `howllo-web`

Enough to support:

- browse
- create post
- vote
- comment
- status display

### Phase 2

Admin.

Build:

- `howllo-admin`

Add:

- moderation
- status updates
- official response
- tags

### Phase 3

Marketing and docs polish.

Build/improve:

- `howllo-landing`
- `howllo-docs`

### Phase 4

AI and commercial/hosted features.

Only after the core feedback loop works in real usage.


## 17. Final Recommended Decision

### Repository names

Use:

- `howllo-server`
- `howllo-web`
- `howllo-admin`
- `howllo-landing`
- `howllo-docs`

### Frontend choice

Use:

- Vite for `howllo-web`
- Vite for `howllo-admin`
- Next.js for `howllo-landing`

### Core principle

Do not force everything into one frontend framework just for uniformity.

Use the right tool for each surface:

- app surfaces -> Vite
- marketing surface -> Next.js

This is the cleanest and most practical direction for Howllo.
