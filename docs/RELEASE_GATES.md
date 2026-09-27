# Howllo 1.0 release gates

Status: **preview, not certified for 1.0**. Updated 2026-09-27. This record
tracks evidence in the repository, not promises inferred from UI screens.
Do not tag `v1.0.0` while any required gate is open.

| Gate | Current evidence | To close the gate |
| --- | --- | --- |
| Core workflow | Boards, posts, votes, comments, moderation, status history and roadmap exist. | Exercise the complete author-to-staff loop, official response, duplicate/merge, archive/lock, search and changelog in a release candidate. |
| Auth without RooIAM | Local accounts and generic OIDC exist. Staff can redeem a single-use invitation code without RooIAM or SMTP. | Remove direct RooIAM imports from core auth paths, use a provider contract, and prove fresh install and every staff/customer login path without RooIAM network or credentials. |
| RBAC and delegation | Owner/admin/moderator/member permissions and invitation accept/reject/withdraw tests exist. | Define and enforce support/staff/viewer capabilities or document the smaller supported role set; certify revocation during active sessions across workspaces. |
| Multi-actor certification | Deterministic API regression and browser persona harness exist; runner reports `NOT_CERTIFIED`. | Run independent accounts concurrently and assert UI, API, DB, audit and realtime invariants. Zero unauthorized writes, cross-tenant leaks, duplicate votes, stale permission bypasses and moderation inconsistencies. |
| One-command self-host | Root Compose starts PostgreSQL and MinIO for development; backend Dockerfile exists. | Ship a versioned production stack with API, App, Web, Admin, storage, health checks, TLS guidance and tested fresh install. |
| Upgrade contract | SQLx migrations and an OpenAPI file exist. Current routes are unversioned `/api/*`. | Test upgrade from a tagged release, define `/api/v1` and deprecation policy, verify OpenAPI against implementation, and version plugin contracts. |
| Observability | Health/readiness endpoints, request IDs, tracing and audit events exist. | Add and test JSON logs, metrics and operator troubleshooting for DB, queues, auth and storage. |
| Security baseline | Rate limits and some isolation tests exist. Private media and browser bearer storage remain open in the security roadmap. | Close critical/high findings, test CSRF/XSS/private uploads, run dependency/secret scans and publish a threat model. |
| Plugin API | First-party approved plugin catalog exists. | Publish a stable, versioned manifest, lifecycle, capabilities, configuration and compatibility test. |
| Data ownership | Post export and backup documentation exist. | Verify full workspace export, restore, deletion and object storage cleanup. |
| Contributor surface | README, architecture, contribution, security and API docs exist. | Add code of conduct, changelog, support policy, templates and a verified contributor setup. |
| License packaging | Root Apache-2.0 LICENSE exists. | Audit bundled dependencies, fonts and art; publish applicable notices and reproducible source/container artifacts. |
| Brand separation | Howllo art is bundled. | Publish an explicit trademark/brand policy reviewed by the maintainer. |
| Contribution policy | CONTRIBUTING exists. | State DCO/CLA choice and verify the contribution workflow before first external PR. |
| Release candidate | No 1.0 RC has been certified. | Run clean install, upgrade, restore, restart, multi-actor, local-only, generic OIDC and RooIAM tracks on an RC; publish results and release artifacts. |

## Release order

1. **0.5 — complete core workflow:** close public board and moderation paths.
2. **0.6 — staff and RBAC:** finish role matrix and delegation certification.
3. **0.7 — provider architecture:** local and generic OIDC work without RooIAM;
   RooIAM remains an optional integration.
4. **0.8 — self-host and upgrade:** one-command stack, restore, stable API and
   plugin contracts.
5. **0.9 — security and certification:** multi-actor test evidence, operations,
   documentation and packaging.
6. **1.0 RC → 1.0:** release only after every gate above has passing evidence.

The immediate blockers are provider independence, self-host/upgrade, and
multi-actor permission certification. Keep the hosted RooIAM configuration
working while making a new self-host installation fully standalone.
