# Isolated deterministic API regression

This runner builds two workspaces through the API and gives each simulated person a unique synthetic RooIAM identity and workspace session. It checks discovered board presets, staff invitations, board visibility, tenant isolation, post permissions, votes, comments, status history, moderation races, database/audit invariants, and a small WebSocket event and isolation scenario. Its database observer uses a read-only PostgreSQL session. The report always says `NOT_CERTIFIED` because comprehensive browser, realtime, and exploratory coverage still requires separate verification.

## Run locally

Build the server, create a **new empty local database**, then run:

```sh
cargo build --manifest-path howllo-server/Cargo.toml
createdb --host=127.0.0.1 --port=5432 --username=postgres howllo_e2e_my_run
HOWLLO_TEST_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/howllo_e2e_my_run \
  node howllo-tests/runner/run.mjs
```

Replace the connection details with those of your disposable local PostgreSQL. The runner refuses remote databases, database names without `test` or `e2e`, nonempty schemas, and occupied API ports. It binds only to `127.0.0.1:7710` by default; `HOWLLO_TEST_PORT` may be 7710–7799. It starts the server from a temporary directory, so the production `.env` is not loaded. No production login or storage service is needed.

Reports are written to the ignored `howllo-tests/reports/` directory. The report contains resource IDs and request IDs, never actor tokens. A zero exit code means **only** that the implemented deterministic API checks passed; `MISSING_CAPABILITY` and `untested_layers` remain visible in the report. Create a new database for every run so the clean-start gate remains meaningful.

Run capability discovery separately with `python3 howllo-tests/discovery/discover.py --json`. Browser checks live in `howllo-tests/browser/`.
