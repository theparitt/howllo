# Howllo system tests

This directory contains three separate test layers:

| Layer | What it verifies | Command |
| --- | --- | --- |
| Capability discovery | Registered routes, board presets, status definitions, and role grants from source | `python3 howllo-tests/discovery/discover.py --json` |
| Deterministic API regression | Multi-person workspace, invitation, permission, moderation, concurrency, database, audit, and basic WebSocket isolation on a fresh local database | `node howllo-tests/runner/run.mjs` |
| Browser smoke | Isolated Playwright contexts and read-only staff/public surfaces | `npm --prefix howllo-tests/browser test` |

See [runner/README.md](runner/README.md) and [browser/README.md](browser/README.md) for setup and safety gates. The CI job runs discovery and the deterministic API layer. The browser layer requires locally running frontends and distinct real actor sessions, so it reports missing prerequisites instead of manufacturing a pass.

## Current certification boundary

`PASS` means the named check passed in its recorded run. It does not mean Howllo is fully certified. The runner report always says `NOT_CERTIFIED` and lists missing capabilities and untested layers. The full specification still calls for a realtime observer, comprehensive UI-to-database scenarios, AI exploratory actors, and every permission/tenant combination. These must be added and verified before claiming full-system certification.

The test identities are synthetic and accepted only by an isolated local API configured for legacy test JWTs. The suite never connects to production, never reuses a production account, and keeps reports free of access tokens.
