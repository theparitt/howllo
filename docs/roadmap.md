# Roadmap

Build sequence (see [internal/ARCHITECTURE.md](internal/ARCHITECTURE.md) §27).

## Phase 1 — core loop ✅

howllo-server + howllo-web: tenant resolution, boards, posts, votes, comments,
status display, Rooiam auth. **Done** (see [PROGRESS.md](PROGRESS.md)).

## Phase 2 — admin (in progress)

howllo-admin (Vite + React): board CRUD, tag CRUD, status updates, moderation
actions, official responses, member roles. Scaffold is in place; wire to the
server API next.

## Phase 3 — landing & docs

Positioning, onboarding, self-hosting docs, OSS docs. Promote howllo-landing to
Next/Astro and howllo-docs to VitePress/Starlight as content grows.

## Phase 4 — AI & advanced

Optional, isolated AI: duplicate suggestions, summaries, tag hints, moderation
assist. Never on the correctness path.

## Later

- `howllo-widget` (port 5115) — embeddable feedback widget, Vite library build.
- Custom domains for boards (`feedback.customer.com`).
- Webhooks, notifications fanout, semantic search.
