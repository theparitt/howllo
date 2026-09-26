# Roadmap to a self-hosted production release

Updated 2026-09-26. This plan comes from a code and documentation review;
release gates below have not yet passed. Older MVP readiness and progress
notes are historical snapshots.

## Product rule and current baseline

Howllo stays a straightforward feedback forum. Topics, replies, search,
categories/tags, pinning, voting, following, notifications, reporting and
staff moderation belong in core. Specialized rendering, themes and integrations
are optional. Authentication, authorization, storage access and spam enforcement
must never depend on tenant CSS or unreviewed plugins.

The repository already has organizations, workspaces, boards, separate staff
App/customer Web/platform Admin views, four board purposes, a public feedback
loop, staff roles and invitations, workspace bans/suspensions, audit logs and
local customer accounts with Argon2id passwords and recovery codes. It has
per-user/per-board post and comment limits, burst cooldowns, quotas, IP/country
rules, manual or automatic post review, and approved appearance plugins.
The first-party plugin registry now rejects unknown catalog rows, and local
operator setup requires a 12-character password. Operator setup and login
share a PostgreSQL-based attempt limit. These changes do not close the other
release blockers below.

## Phase 0 — release blockers: identity, privacy and abuse

1. **Protect browser sessions.** Replace JavaScript-readable bearer tokens in
   localStorage and cookies with origin-scoped `HttpOnly`, `Secure`, `SameSite`
   sessions or a backend-for-frontend. Add CSRF protection for cookie writes,
   session lists/revocation, and tests for logout, role changes and expiry.
2. **Fix private media semantics.** The upload path stores assets through
   `store_public_asset`, and MinIO preflight makes its bucket public-read.
   Private-board attachments need a private object path and authenticated
   download or short-lived signed URLs. Audit existing private content before
   promising that all board data is private. New private-board screenshot
   uploads and attachment references are currently rejected; existing public
   object URLs attached to private posts still require an audit and migration.
3. **Make rate limits work behind proxies and across replicas.** Local auth has
   a ten-per-minute in-memory limit keyed by TCP peer. Behind one reverse proxy
   that peer may be shared by all customers; process restart/replicas reset the
   count. The general public-write IP limiter also defaults to disabled,
   though workspace account/board limits still apply. Resolve real IP only
   through a configured trusted proxy, then use a
   shared limiter for login, signup, recovery and writes. Cover account and IP
   dimensions, including platform-admin login.
4. **Harden privileged login.** Increase the eight-character local operator
   password minimum, add MFA/passkeys or require administrator OIDC with MFA,
   and require fresh authentication for sensitive settings. Provide local or
   generic-OIDC staff onboarding for self-hosting; RooIAM stays optional. Do
   not merge provider identities merely because emails match. For staff
   invitations, require a verified provider email or a single-use invite link;
   local customer accounts currently use usernames without verified email.
5. **Exercise isolation.** Add cross-workspace permission, private media,
   tenant OIDC secret, restriction and revoked-session tests, plus a manual
   browser pass for all three login levels.

**Exit gate:** a fresh install creates its first staff account and workspace
without RooIAM; no private attachment is anonymously readable; proxy-aware
abuse controls and revocation work with multiple server instances; no known
critical/high security issue remains open.

## Phase 1 — daily forum use and humane moderation

1. Give visitors a **Report** action on topics and replies, with reasons,
   duplicate-report suppression, a staff queue, decisions and audit trail.
2. Add new-account probation: first one or two posts may await review, then
   approved users publish normally. Combine link volume, repeated text and
   duplicate-topic signals into an explainable score. Tell authors when a post
   is pending; do not silently discard it.
3. Add an optional **adaptive** bot challenge for signup, recovery and
   suspicious writes. For Turnstile, verify tokens server-side through
   Siteverify. Keep a non-Cloudflare path for self-hosters and do not challenge
   every ordinary reply.
4. Improve staff workflow: bulk review, reasons/templates, short suspension,
   appeals, blocked-word/URL lists, moderator notes and simple queue metrics.
   Test false positives and allow a staff override.
5. Complete forum details: edit/delete own replies with short history, clear
   unread markers, follow controls, mobile and keyboard usability, accessible
   forms and simple visitor navigation.

**Exit gate:** a tenant runs a public board for one week without approving
every normal post, can handle reports/appeals, and can explain delayed writes.
Measure queue age, false positives and 429 responses.

## Phase 2 — self-hosting and open-source operations

1. Ship one versioned Docker Compose reference stack for API, App, Web, Admin,
   PostgreSQL and MinIO/local storage. Include local-staff setup, TLS reverse
   proxy, env validation, health checks, resource limits and an upgrade guide.
   Landing and docs are optional services.
2. Document backup **and tested restore** of PostgreSQL, objects and
   encryption keys. Automate clean-install and upgrade smoke tests; define
   migration rollback policy and release tags/changelog.
3. Add CI for Rust/SQLx tests, frontend builds/type checks, migration tests,
   dependency/security scans and secret scanning. Add root `SECURITY.md`,
   contribution guide, issue templates and support policy.
4. Add metrics/alerts for API errors and latency, auth failures, moderation
   queue age, storage use, failed webhooks and backup age. Document retention
   and deletion for accounts, IP/user-agent logs and uploads.

**Exit gate:** a new maintainer deploys from the published docs, upgrades
between two tagged releases and restores a backup to a new environment,
without a Cloudflare or RooIAM account.

## Phase 3 — first-party optional extensions

Only the Howllo maintainer may author and ship plugins. The platform admin
approves each built-in plugin; tenants can only enable approved choices per
workspace or board. There are no outside submissions or external plugin URLs.

Extend the approved plugin catalog with a versioned manifest, board scope,
compatibility range, required public API version, permissions, CSP policy and
disable/rollback behavior. Never execute arbitrary tenant JavaScript in the
board origin.

Early first-party plugins: sanitized Markdown with fenced-code highlighting, polls,
accepted answers for support boards, emoji reactions, richer themes and custom
navigation/footer links. Later: external spam scoring, chat notifications,
embeddable widget and AI suggestions. Security-sensitive OIDC and storage
integrations remain reviewed server adapters.

**Exit gate:** a plugin can be installed, enabled for one board, upgraded,
disabled and removed without affecting another board or leaving unsafe HTML
or orphaned data. Core posting works with all plugins disabled.

## Phase 4 — scale after real usage

Use measured demand to decide on custom domains, richer search, email digests,
large-community moderation tools and horizontal scaling. Do not add AI or a
outside plugin marketplace. Keep new functionality in core or reviewed
first-party plugins after the release and moderation gates pass.

## References used for release criteria

- [OWASP session management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
- [OWASP authentication](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)
- [Cloudflare Turnstile server validation](https://developers.cloudflare.com/turnstile/get-started/server-side-validation/)
- [Discourse spam guidance](https://meta.discourse.org/t/tips-for-preventing-spam/264020)
- [GitHub vulnerability reporting](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting)
