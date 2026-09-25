# Howllo MVP readiness (2026-09-25)

## Product scope

Howllo uses `accounts` for the customer organization, `tenants` for its
workspaces, and `boards` for feedback boards inside each workspace. An account
owner can create multiple workspaces; a workspace owner or admin can create and
manage multiple boards. Public users can submit posts, vote, comment, and
follow. Staff can moderate posts, manage statuses, and publish a roadmap.

| Core capability | Current implementation |
| --- | --- |
| Organization with multiple workspaces | Account membership and workspace creation APIs; account owner can access owned workspaces |
| Multiple boards per workspace | Board CRUD in server and admin; database uniqueness is scoped to the workspace |
| Public feedback loop | Board pages, post creation, voting, comments, following, and status display |
| Staff workflow | Local admin console and workspace management screens for boards, moderation, roadmap, members, and branding |
| Data and media | PostgreSQL for records, MinIO for uploaded images, with a public bucket URL |

## Verified on the current local setup

- Backend starts on `127.0.0.1:7700` using PostgreSQL and MinIO at
  `192.168.0.147`. All 28 migrations are applied.
- The live database contains two workspaces with four and three boards. Both
  public board lists, dashboards, feeds, and sample board pages return HTTP 200.
- Backend startup confirms the MinIO bucket accepts an upload, anonymous read,
  and cleanup. Saved `system_settings` select the MinIO backend and the public
  bucket URL; these settings override storage defaults in `.env`.
- Backend CORS accepts the App, Web, and Admin origins (`7702`, `7703`, and `7701`).
- App, Web, and Admin production builds pass. The admin login and web sign-in screens
  render in Chromium without an API connection error.
- RooIAM workspace settings now provide a client ID, widget URL, and RooIAM
  workspace ID for both Howllo workspaces. The web token exchange runs on the
  Next.js server because the RooIAM token endpoint rejects browser preflight
  requests from the local web origin. Invalid-code requests reach RooIAM and
  return its authorization-code error.
- The backend integration suite passed 101 tests against an isolated test
  database, including workspace creation, board management, posting,
  permissions, moderation, and workspace sessions.
- The schema links workspaces to accounts and enforces unique board slugs per
  workspace with `UNIQUE (tenant_id, slug)`.

## Still requiring a real account check

- The tenant App uses port `7702` and opens the RooIAM widget on its account home; the public Web uses port `7703` and local accounts. RooIAM callback registration must match the App origin. A complete login still requires a real RooIAM account check.
- Complete a RooIAM browser login and
  confirm a post, vote, comment, and workspace management action using a real
  user. Automated checks cannot provide that user's credentials.
- Log into the admin console with the existing local admin password and verify
  its board form. The live database reports that admin login is bootstrapped;
  the password was not changed during this review.
- For access from other machines, set browser-reachable API URLs and CORS
  origins, and bind the server and Vite/Next apps on LAN interfaces or place
  them behind a reverse proxy. The current setup is local-host oriented.

Features such as custom domains, email delivery, and external AI providers are
outside this MVP scope.
