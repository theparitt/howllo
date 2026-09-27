# Howllo browser smoke tests

This is a read-only, local-only Playwright layer. It checks browser-context isolation, the public board directory and search, the staff app sign-in surface, the admin sign-in gate, and an optional positive/negative private-board access control. It does **not** certify the full-system specification or run destructive scenarios.

## Run

```bash
cd howllo-tests/browser
npm ci
npx playwright install chromium
npm test
```

Start the local API, admin UI, staff app, and public web before running the tests. URL defaults are `http://127.0.0.1:7700`, `:7701`, `:7702`, and `:7703`. Override them with `HOWLLO_TEST_API_URL`, `HOWLLO_TEST_ADMIN_URL`, `HOWLLO_TEST_APP_URL`, and `HOWLLO_TEST_WEB_URL` if needed. Only HTTP loopback origins are accepted, so this suite cannot accidentally browse a production deployment. Configure the local Next/Vite instances to use the local API too.

Set `HOWLLO_TEST_WORKSPACE_A` to an existing, published local workspace. The suite discovers all published boards from its public directory; no board type list is hardcoded in the browser checks. Set `HOWLLO_TEST_PRIVATE_BOARD_A` to an existing private board slug for the isolation check. That check requires the owner to see the board first, then requires the foreign account and guest to receive 404, so a nonexistent board cannot create a false pass.

Optional authenticated actor state lives **outside the repository** in the folder named by `HOWLLO_TEST_PERSONAS_DIR`:

```text
A-OWNER.storage.json
A-MOD.storage.json
U-FEATURE.storage.json
U-FOREIGN.storage.json
```

Each file is a Playwright `browserContext.storageState()` snapshot from that actor's own local login. Capture it using the same `localhost` or `127.0.0.1` origin configured for the test. Use distinct accounts, not one admin login copied into multiple files. These files contain credentials; keep them private. The suite rejects states with a shared credential and loads only loopback cookies and storage into each context; external RooIAM cookies are discarded. `A-OWNER` is a staff owner in workspace A; `U-FEATURE` is an end user; `U-FOREIGN` belongs only to workspace B. `A-MOD` is reserved for role-specific scenarios. No test logs or report entries include credential values. Failure traces under `test-results/` can contain session data and must also be kept private.

The result file is `reports/summary.json`. `PASS`, `FAIL`, and `MISSING_CAPABILITY` are separate outcomes. Missing services, actor states, published boards, or a configured private board never count as pass: `npm test` exits nonzero when any browser case is incomplete. The report always says `NOT_CERTIFIED`; full certification needs the API/database/audit/realtime layers and the mutation scenarios from the full-system specification.

The suite does not create accounts or write application data. The search test submits a read-only query with a unique marker. It deliberately avoids following external RooIAM login links.
