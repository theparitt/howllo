# Howllo Architecture

## 1. Overview

Howllo is a standalone multi-tenant feedback and roadmap platform.

It is designed around a clear separation of concerns:

- `howllo-server` owns backend business logic and data integrity
- `howllo-web` owns the public product experience
- `howllo-admin` owns internal moderation and tenant administration
- `howllo-landing` owns marketing and acquisition
- `howllo-docs` owns product and self-hosting documentation

The system should be backend-centric.

This means:

- business rules live in the server
- frontends are thin clients
- tenant isolation is enforced in the backend
- AI is an optional assistive layer, never the source of truth


## 2. Architecture Goals

The architecture should optimize for:

- clear tenant boundaries
- fast iteration
- low accidental complexity
- strong backend ownership of rules
- ability to scale from internal use to hosted SaaS
- ability to remain useful without AI
- ability to dogfood Rooiam authentication cleanly

The architecture should avoid:

- putting business logic inside UI code
- mixing marketing and app concerns
- over-abstracting too early
- building heavy microservices too early
- making AI part of the correctness path


## 3. System Context

Howllo sits in the product ecosystem as:

- local accounts or configured OIDC providers = identity providers; RooIAM is optional
- Howllo = feedback and roadmap platform

At runtime:

- user visits Howllo UI
- Howllo UI calls `howllo-server`
- `howllo-server` validates Howllo sessions or a configured provider flow
- `howllo-server` executes domain logic
- PostgreSQL stores tenant-scoped data
- optional AI services enhance search, duplicate detection, summaries, and moderation assistance later


## 4. High-Level Components

## `howllo-server`

Primary backend application.

Responsibilities:

- API
- auth/session integration
- tenant isolation
- board logic
- post logic
- comment logic
- vote logic
- moderation
- status workflow
- search coordination
- AI assistance orchestration later

This is the authoritative system for product rules.


## `howllo-web`

Public/product-facing frontend.

Responsibilities:

- browse boards
- view posts
- create signals
- comment
- vote
- filter and sort
- show roadmap status
- sign-in entry flow

This is the primary end-user application.


## `howllo-admin`

Admin and moderation frontend.

Responsibilities:

- tenant settings
- board settings
- tags
- status updates
- official responses
- post moderation
- duplicate handling later
- admin analytics later

This is intentionally separate from `howllo-web`.


## `howllo-landing`

Marketing surface.

Responsibilities:

- home page
- features
- pricing
- open source page
- sign-up / CTA pages
- brand and positioning

This should not own product business logic.


## `howllo-docs`

Documentation surface.

Responsibilities:

- setup docs
- self-hosting docs
- API docs
- auth integration docs
- product concepts
- hosted vs open-core boundary explanation later


## 5. Deployment Topology

Current deployment surfaces:

- `howllo.dev` and `www.howllo.dev` -> landing Worker
- `app.howllo.dev` -> tenant management Worker
- `feedback.howllo.dev` -> public web Worker
- `admin.howllo.dev` -> platform admin Worker
- `api.howllo.dev` -> Cloudflare Tunnel to the Rust server and public MinIO assets

### Local development port allocation

Canonical map (also in `shared/config/src/ports.ts`):

- `7700` -> `howllo-server` HTTP API and `/ws` realtime endpoint
- `7701` -> `howllo-admin` (Vite + React) platform operations
- `7702` -> `howllo-app` (Next.js) workspace management
- `7703` -> `howllo-web` (Next.js) public feedback boards
- `5114` -> `howllo-landing` marketing site
- `5115` -> `howllo-widget` (reserved, later)
- `5116` -> `howllo-docs` help / documentation surface

Important rule:

- `7701` runs the platform operator surface
- board realtime socket traffic uses `7700/ws`
- user help and product documentation live on `5116`

Recommended infrastructure:

- reverse proxy
- PostgreSQL
- one backend service for `howllo-server`
- static/frontend hosting for `howllo-web`
- static/frontend hosting for `howllo-admin`
- Next.js hosting/runtime for `howllo-landing`
- docs hosting/runtime for `howllo-docs`

### Early-stage deployment rule

Start simple.

Do not split the backend into multiple services early.
A well-structured modular monolith is the correct starting architecture.


## 6. Backend Architecture Style

Recommended backend style:

- modular monolith
- domain-oriented modules
- thin HTTP handlers
- explicit service layer
- repository/query separation where useful
- transaction control in backend domain/service layer
- DTO separation from domain types

This gives:

- clarity
- strong Rust typing
- fewer accidental cross-module leaks
- easier evolution into separate services later if ever needed


## 7. `howllo-server` Internal Structure

Suggested crate/module structure:

```text
src/
    main.rs
    lib.rs

    config/
    startup/
    http/
    auth/
    tenancy/
    users/
    memberships/
    boards/
    posts/
    comments/
    votes/
    tags/
    statuses/
    moderation/
    notifications/
    search/
    ai/
    db/
    models/
    dto/
    errors/
    shared/

This is a conceptual structure.
Actual submodule names can be adjusted, but the separation should remain.

8. Layered Responsibility Model

Howllo server should roughly follow this responsibility model:

HTTP layer

Responsibilities:

parse requests
extract auth/session context
validate basic request shape
call application/domain services
map errors to HTTP responses
serialize responses

This layer should not contain business rules.

Application/domain layer

Responsibilities:

execute use cases
enforce workflow rules
check authorization
coordinate transactions
call repositories/query layer
coordinate search/AI assistance when needed

This layer owns the platform behavior.

Persistence/query layer

Responsibilities:

SQLx queries
row mapping
persistence details
read-model shaping where appropriate

This layer should not decide product behavior.

Shared layer

Responsibilities:

common IDs
timestamps
pagination primitives
shared validation helpers
common error helpers
9. Example Folder Structure

A more concrete example:

src/
    main.rs
    lib.rs

    config/
        mod.rs
        settings.rs

    startup/
        mod.rs
        app.rs
        server.rs
        routes.rs

    http/
        mod.rs
        extractors.rs
        responders.rs
        pagination.rs

    auth/
        mod.rs
        current_user.rs
        rooiam.rs
        permissions.rs

    tenancy/
        mod.rs
        tenant.rs
        tenant_service.rs
        tenant_repository.rs

    boards/
        mod.rs
        board.rs
        board_service.rs
        board_repository.rs
        board_queries.rs

    posts/
        mod.rs
        post.rs
        post_service.rs
        post_repository.rs
        post_queries.rs
        post_validation.rs

    comments/
        mod.rs
        comment.rs
        comment_service.rs
        comment_repository.rs

    votes/
        mod.rs
        vote_service.rs
        vote_repository.rs

    tags/
        mod.rs
        tag.rs
        tag_service.rs
        tag_repository.rs

    statuses/
        mod.rs
        status.rs
        status_service.rs
        status_history_repository.rs

    moderation/
        mod.rs
        moderation_service.rs
        moderation_repository.rs

    search/
        mod.rs
        search_service.rs
        related_posts.rs

    ai/
        mod.rs
        completion.rs
        embeddings.rs
        summarization.rs
        duplicate_suggestions.rs

    db/
        mod.rs
        pool.rs
        transaction.rs

    dto/
        mod.rs
        common.rs
        boards.rs
        posts.rs
        comments.rs
        tags.rs

    models/
        mod.rs
        ids.rs
        pagination.rs

    errors/
        mod.rs
        app_error.rs
        error_codes.rs

    shared/
        mod.rs
        slug.rs
        time.rs
        text.rs

This is not a strict requirement, but it is a strong starting point.

10. Request Flow

Example flow for creating a post:

request arrives at HTTP route
handler parses body and auth context
handler calls post service
post service:
validates tenant access
validates board rules
validates post content
opens transaction if needed
inserts post
inserts tags if needed
commits transaction
service returns domain result
handler maps it to API response

Important rule:

HTTP handler should not directly run all SQL or own the full workflow.

11. Domain Modules
tenancy

Responsibilities:

tenant lookup
tenant slug resolution
tenant-level settings
tenant activation/suspension later
tenant policy boundaries

The tenancy module is foundational because most entities are tenant-scoped.

users

Responsibilities:

local user record
mapping from provider ID and subject to the Howllo user ID
user profile basics
user lifecycle inside Howllo context

RooIAM remains an optional provider. Howllo stores provider-neutral user records.

memberships

Responsibilities:

relationship between user and tenant
tenant roles
membership status
invite acceptance later if needed

This should remain separate from authentication.

boards

Responsibilities:

board CRUD
board ordering
board visibility
board type
board-level constraints

Examples of board types:

feature-requests
bug-reports
discussions
announcements
posts

Responsibilities:

create/update posts
publish state if needed later
slug or identifier generation
tenant ownership
board association
duplicate linking later
post-level validation

This is one of the core domains.

comments

Responsibilities:

create comments
comment visibility
comment moderation
threaded comments later if added

Start with flat comments unless true nesting becomes necessary.

votes

Responsibilities:

add/remove vote
enforce one vote per user per post
compute vote counts safely
support later reaction models if ever needed

Keep this deterministic and simple.

tags

Responsibilities:

tenant-scoped tag management
post tagging
tag validation
filtering support
statuses

Responsibilities:

status transitions
status history
roadmap states
admin-controlled workflow

Suggested first statuses:

under-review
planned
in-progress
done
declined
moderation

Responsibilities:

hide post
lock post
mark official response
mark duplicate later
moderation audit data later

This should remain clearly distinct from normal user content operations.

search

Responsibilities:

full-text search coordination
filtering integration
semantic related-post support later
duplicate-candidate lookup later

This should initially work without AI.

ai

Responsibilities:

duplicate suggestion
summarization
tag suggestion
classification assistance
moderation assistance

This module must be optional and isolated.

12. Data Ownership Rules

Important rule:

Each domain owns its logic.

Examples:

posts module owns post creation rules
votes module owns vote invariants
statuses module owns status transition rules
moderation module owns moderation actions

Do not scatter logic across many unrelated modules.

Do not let UI-specific assumptions leak into domain modules.

13. Multi-Tenancy Model

Howllo should be multi-tenant from the beginning.

Tenant boundary rule

Every major tenant-owned entity should carry tenant_id.

Suggested entities with direct tenant_id:

boards
posts
tags
memberships
tenant settings
board settings
status configuration later
notifications later

Comments may use post ownership transitively, but adding tenant_id directly can also simplify queries and enforcement.

Why this matters

It ensures:

easier filtering
easier authorization
easier indexing
clearer operational debugging
fewer accidental cross-tenant leaks
Non-negotiable rule

Tenant scoping must be enforced in the backend, not trusted from the frontend.

14. Authentication and Authorization
Authentication

Local accounts and configured OIDC providers authenticate users. RooIAM is optional.

Howllo should accept:

user identity from local credentials or an OIDC authorization code flow
local mapping from `(provider_id, subject)` to an internal user

Howllo validates its own sessions and provider responses before granting access.

Authorization

Howllo owns authorization.

Meaning:

The configured provider establishes who the user is
Howllo says what the user can do in a tenant

Authorization should check:

tenant membership
tenant role
board visibility
action-level permission
Suggested early roles
owner
admin
moderator
member

Keep the role model simple in V1.

15. Database Direction

Use PostgreSQL.

Use SQLx.

General database rules
prefer explicit SQL
keep migrations readable
enforce constraints in schema where possible
use indexes intentionally
use unique constraints for invariants
use foreign keys where practical
Example critical invariants
one vote per ( post_id, user_id )
unique board slug per tenant
unique tag slug per tenant
unique membership per ( tenant_id, user_id )
Suggested baseline tables
tenants
users
memberships
boards
posts
comments
post_votes
tags
post_tags
post_status_history

Optional later:

official_responses
duplicate_links
notifications
audit_entries
tenant_settings
board_settings
16. Read/Write Patterns

Start with straightforward transactional writes and direct query reads.

Write path

Use service/domain layer plus repository or SQLx query calls.

Read path

Can remain simple initially.
You do not need separate CQRS infrastructure in V1.

If needed later, build optimized read queries for:

trending lists
roadmap views
tenant dashboards
admin review queues

But keep the initial design simple.

17. API Design

Use REST-first JSON APIs.

Suggested route groups
/v1/auth/*
/v1/tenants/*
/v1/boards/*
/v1/posts/*
/v1/comments/*
/v1/votes/*
/v1/tags/*
/v1/admin/*
Principles
explicit resource naming
tenant-aware filtering
strong validation
predictable pagination
no over-designed generic endpoints
Pagination

Use cursor or stable page-based pagination.
Cursor-based is better long-term for active feeds, but page-based is acceptable for V1 if simpler.

Response structure

Use consistent envelope style only if it adds value.
Do not over-nest responses unnecessarily.

18. Error Handling

Centralize application errors.

Suggested categories:

validation error
unauthorized
forbidden
not found
conflict
rate limited
internal error

Map them predictably to HTTP responses.

Do not leak internal database errors directly to clients.

Error design principle

Domain errors should be meaningful.
Handlers should map them, not invent them ad hoc.

19. Validation Rules

Validation should happen in multiple layers:

Request-shape validation

At HTTP boundary.

Examples:

missing fields
invalid type
obvious malformed values
Domain validation

At service/domain layer.

Examples:

post title too short
post body too long
user not allowed in tenant
board locked
duplicate vote
invalid status transition

This split keeps code clearer.

20. Search Architecture
V1

Use PostgreSQL full-text search and normal indexed filters.

Capabilities:

title/body search
board filter
tag filter
status filter
sort by top/newest/active
Later

Add semantic search as an enhancement.

Possible stages:

embeddings for related posts
duplicate detection
semantic search
cluster suggestions

But V1 must work well without embeddings.

21. AI / LLM Architecture

AI must be optional and isolated from correctness.

AI role

AI should assist with:

related post suggestion
duplicate candidates
thread summaries
tag suggestions
classification hints
moderation hints
AI rule

AI suggests.
Humans or deterministic rules decide important state changes.

Suggested abstraction

Use traits/interfaces like:

EmbeddingProvider
CompletionProvider
ModerationAssistProvider

This keeps vendor choice flexible and prevents model lock-in.

Suggested AI flow example

For duplicate suggestions:

new post draft enters service
service asks search/embedding layer for similar posts
optional LLM reranks or summarizes similarity
frontend shows suggestions
user decides whether to continue or open an existing post

Important:

The original post flow must still work even if AI is disabled or failing.

22. Events and Background Work

Do not introduce a large event-driven architecture in V1.

Start with synchronous core flow

Good for:

create post
comment
vote
update status
Add background jobs only when needed

Examples later:

notification fanout
summary generation
duplicate clustering refresh
trend computation
search indexing refresh if external search is added

If needed later, add a worker process rather than redesigning the whole platform early.

23. Caching

Start with minimal caching.

Likely enough for early stage:

database indexes
reverse proxy/static caching for frontend assets

Add backend caching only after clear bottlenecks appear.

Do not pre-optimize aggressively.

24. Observability

At minimum, support:

structured logs
request IDs
error logs
SQL timing visibility if possible
key action logs for moderation/status changes

Later add:

metrics
tracing
tenant-level usage metrics
AI usage metrics

This is especially important once hosted SaaS starts growing.

25. Frontend Architecture Principles
howllo-web

Recommended frontend structure:

route-based pages
API client layer
shared UI components
feature-oriented modules where useful
server as source of truth

Suggested broad structure:

src/
    app/
    routes/
    pages/
    features/
    components/
    hooks/
    api/
    types/
    lib/
Principle

Do not duplicate backend workflow rules inside the frontend.
Frontend can do UI validation, but server remains authoritative.

howllo-admin

Use a similar structure, but with admin-specific feature modules.

Examples:

moderation queue
board settings
status management
tenant settings
tag management

Keep it separate so admin complexity does not leak into public UX.

26. Security Principles

Important security rules:

all tenant access checked server-side
all moderation actions authorized server-side
never trust frontend role claims
sanitize and validate user-generated text
rate limit post/comment creation
protect admin surfaces separately
log important admin/moderation actions

As a product handling user content, moderation and abuse controls matter early.

27. Recommended Build Sequence
Phase 1

Build howllo-server and howllo-web.

Focus on:

tenant resolution
board list
post list
post detail
create post
comment
vote
auth integration
status display
Phase 2

Build howllo-admin.

Focus on:

board CRUD
tag CRUD
status update
moderation actions
official response
membership roles if needed
Phase 3

Improve landing and docs.

Focus on:

positioning
onboarding
self-hosting docs
hosted SaaS docs
OSS docs
Phase 4

Add AI and advanced hosted features.

Only after the core workflow is used in reality.

28. Architecture Decisions Summary
Decision 1

Use a modular monolith backend.

Reason:
Simpler, faster, clearer, enough for current scope.

Decision 2

Keep business logic in howllo-server.

Reason:
Consistency, security, tenant safety.

Decision 3

Use local accounts or configured OIDC providers for authentication, and Howllo for authorization.

Reason:
Clean separation of identity vs permissions.

Decision 4

Be multi-tenant in architecture from day one.

Reason:
Future reuse across products and hosted SaaS path.

Decision 5

Keep AI optional and assistive.

Reason:
Stability, cost control, product clarity.

Decision 6

Separate product UI, admin UI, landing, and docs.

Reason:
Cleaner boundaries and easier evolution.

29. Final Guiding Principle

Howllo should be built as a clear, backend-owned, multi-tenant feedback platform.

The system should remain understandable even as features grow.

If a new feature makes the architecture harder to reason about, ask:

does it improve the feedback-to-roadmap loop
does it reduce real user or admin pain
does it belong in the core platform now

If not, defer it.
