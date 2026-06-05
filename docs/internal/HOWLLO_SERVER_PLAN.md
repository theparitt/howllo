# Howllo Server Plan

## Purpose

This document turns the overall Howllo product direction into a concrete execution plan for `howllo-server`.

It answers:

- what the backend must own
- which goals are in scope for each phase
- what is already done
- what remains before the server can fully support the product vision
- how to sequence the work without adding accidental complexity


## Guiding Rule

`howllo-server` is the source of truth for:

- tenant isolation
- permissions
- workflow transitions
- moderation rules
- roadmap state visibility
- data integrity
- auditability of important admin actions

If a frontend can bypass a rule by calling the API differently, the backend plan is incomplete.


## Current State

As of June 2, 2026, the server already provides:

- PostgreSQL schema for tenants, users, memberships, boards, posts, comments, votes, tags, and post status history
- Rooiam-backed authenticated user extraction
- public board listing
- public post listing and post detail
- authenticated post creation
- authenticated comment creation and comment listing
- vote add/remove endpoints
- admin endpoints for post status, post visibility, post lock, and official comment marking
- modular monolith structure aligned with the intended architecture

This is enough for a basic MVP loop, but not enough for the full product goals described in the product plan.


## Server Goals

The backend must fully support the feedback-to-roadmap loop:

1. Accept signals cleanly.
2. Enforce tenant and membership boundaries.
3. Let product teams review and moderate signals safely.
4. Turn signals into visible roadmap progress.
5. Preserve trust through explicit status history and official responses.
6. Stay operationally simple enough for self-hosting and early SaaS use.


## Non-Goals For Early Phases

These should not delay the core server roadmap:

- plugin system
- complex enterprise RBAC matrix
- deep theming APIs
- custom workflow builders
- AI-driven autonomous moderation
- heavy notification infrastructure
- multi-region architecture
- broad analytics warehouse work


## Workstreams

The server roadmap should be managed across these workstreams:

1. Core domain APIs
2. Authorization and tenant safety
3. Moderation and roadmap workflow
4. Admin operating surface
5. Search and discovery
6. Auditability and reliability
7. AI-assisted features
8. Delivery and operations


## Phase 0: Stabilize The Existing MVP

### Goal

Make the current backend trustworthy enough to build the rest of the product on top of it.

### Deliverables

- centralize config loading instead of relying on scattered `env::var` calls
- introduce a consistent error model and HTTP error mapping
- add request validation for create/update payloads
- add structured logging and request IDs
- move business rules out of handlers where they are currently embedded
- define canonical status enum values instead of freeform strings
- add integration tests for existing endpoints
- enforce SQLx compile-time query checking in CI

### Exit Criteria

- all current endpoints have happy-path and permission tests
- status values cannot drift due to typo-based writes
- runtime failures return predictable API errors
- logs are sufficient to debug request failures without reproducing locally


## Phase 1: Complete The Core Feedback Loop

### Goal

Cover the full V1 feedback workflow in the backend, not just the minimal read/write path.

### Deliverables

- board detail endpoint
- create/edit board endpoints for admins
- richer post listing filters:
  - `status`
  - `tag`
  - `sort`
  - pagination
- post update endpoint for authors within allowed rules
- post soft-delete or hide flow with explicit policy
- tag CRUD endpoints for admins
- attach/detach tags from posts
- status history read endpoint on post detail
- official team response endpoint or response metadata strategy
- membership-aware visibility checks for private boards

### Exit Criteria

- a tenant admin can configure boards and tags entirely through API
- end users can browse, submit, and follow feedback without frontend-only logic
- roadmap-relevant state is visible via API, including history
- private/public board rules are enforced only by backend authorization


## Phase 2: Moderation And Roadmap Operations

### Goal

Make the backend capable of supporting real product-team workflows rather than a simple public board.

### Deliverables

- explicit moderation note model or internal action log
- duplicate-post workflow:
  - mark duplicate
  - point to canonical post
  - preserve vote/discussion handling policy
- status transition guardrails:
  - allowed transitions
  - optional admin note
  - actor tracking
- comment moderation endpoints:
  - hide/unhide
  - lock policy behavior
- board-level moderation filters:
  - hidden
  - locked
  - under review
  - planned
  - in progress
  - done
  - declined
- tenant membership management endpoints for owner/admin roles
- roadmap-focused list endpoints for public consumption

### Exit Criteria

- moderators can process noisy input without direct database access
- roadmap progression is auditable and consistent
- duplicate handling no longer requires manual convention in the UI
- admin workflows no longer depend on hidden frontend assumptions


## Phase 3: Production Readiness For Open Core

### Goal

Make `howllo-server` stable, self-hostable, and safe enough to be the product’s public backend foundation.

### Deliverables

- API versioning strategy
- OpenAPI or equivalent machine-readable API documentation
- health, readiness, and dependency checks
- migration discipline:
  - reversible where practical
  - seeded dev data strategy
- rate limiting for write-heavy endpoints
- abuse protection for post/comment/vote creation
- CORS and trusted-origin policy
- idempotency strategy where needed for admin actions
- audit log model for sensitive mutations
- background job approach for future notifications and AI tasks
- deployment documentation for Postgres + backend service

### Exit Criteria

- a third party can self-host the backend from docs without reverse engineering
- operational failures are visible through health and logs
- sensitive admin actions are attributable
- public write endpoints have basic abuse controls


## Phase 4: Search, Discovery, And Roadmap Quality

### Goal

Improve retrieval and signal organization so the product scales beyond a small internal board.

### Deliverables

- search endpoints for boards and posts
- full-text search in PostgreSQL as baseline
- ranking strategy for:
  - top
  - newest
  - active
  - most voted
- suggested duplicate candidates during post creation
- tag-based and status-based roadmap views
- board summaries and lightweight aggregation endpoints

### Exit Criteria

- users can find existing feedback before submitting duplicates
- product teams can review signals by importance, freshness, and roadmap state
- discovery quality is meaningfully better without requiring AI


## Phase 5: AI Assistance Layer

### Goal

Add AI only where it assists human workflows and never becomes the correctness path.

### Deliverables

- duplicate suggestion service behind feature flag
- sentiment/topic clustering for admin review
- summarization of long discussion threads
- AI-generated moderation suggestions with explicit human approval
- AI-generated roadmap grouping suggestions
- evaluation dataset and human-review loop for false positives

### Exit Criteria

- AI outputs are advisory only
- every AI action has a clear review surface
- the product remains fully usable with AI disabled


## Cross-Phase Technical Decisions

These decisions should stay stable across phases.

### 1. Status Model

Define status as a backend enum with a closed set, such as:

- `under_review`
- `planned`
- `in_progress`
- `done`
- `declined`

If public copy needs hyphenated or title-case labels, that mapping belongs in DTOs or frontend presentation, not the database contract.

### 2. Tenant Resolution

All read and write paths must remain tenant-scoped.

Recommended rule:

- public routes resolve tenant by slug
- admin routes resolve tenant through the target record and verify membership
- no write path accepts untrusted tenant ownership without re-checking from the database

### 3. Authorization Model

Keep the early role set simple:

- `owner`
- `admin`
- `moderator`
- `member`

Do not introduce fine-grained per-action permissions until actual product needs force it.

### 4. API Style

Stay backend-centric and explicit:

- thin handlers
- service-oriented workflow logic
- DTO separation from database models
- transaction boundaries in application logic

### 5. Comment Model

Threaded comments are a later-phase extension.

Do not fake threading in the frontend before the backend model exists. When implemented, use explicit parent linkage and permission checks rather than UI-only nesting.


## Recommended Execution Order

The safest order is:

1. Phase 0
2. Phase 1
3. Phase 2
4. Phase 3
5. Phase 4
6. Phase 5

Reason:

- Phase 0 reduces rework.
- Phase 1 completes the basic product contract.
- Phase 2 enables real moderation and roadmap operation.
- Phase 3 makes the backend fit for open-core usage.
- Phase 4 improves scale and quality.
- Phase 5 adds optional intelligence last.


## Immediate Next Sprint

If work starts now, the next backend sprint should focus only on the highest-leverage gaps:

1. Introduce typed status values and centralized error handling.
2. Add integration tests for current endpoints.
3. Add pagination, filtering, and sorting to post listing.
4. Add status history read APIs.
5. Add tag CRUD plus post-tag assignment.
6. Add comment moderation and duplicate-handling groundwork.

This sequence improves backend correctness and unlocks both `howllo-web` and `howllo-admin` without broad architecture churn.


## Definition Of Done For `howllo-server` V1

`howllo-server` should be considered V1-complete when all of the following are true:

- tenants, boards, posts, comments, votes, tags, and statuses are fully manageable through API
- public and private visibility rules are enforced in the backend
- admin moderation workflows are complete enough for daily product-team use
- roadmap progress is visible and historically traceable
- the backend is documented, test-covered, and self-hostable
- search is good enough to reduce duplicate submissions
- AI remains optional and non-authoritative


## Summary

The backend is already a credible MVP foundation, but it is only partway to the full product plan.

The correct path is not to add more surfaces first. The correct path is to harden `howllo-server`, complete the core workflow, then add moderation depth, production readiness, discovery, and finally AI assistance.
