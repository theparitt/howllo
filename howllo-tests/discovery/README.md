# Capability discovery

Run before selecting deterministic scenarios:

```sh
python3 howllo-tests/discovery/discover.py --json > capabilities.json
python3 -m unittest discover -s howllo-tests/discovery -v
```

`board_types.frontend_presets` is the current customer UI board matrix. It is
parsed from the preset source; adding a fifth preset adds it to the next run.
The database column accepts free-form values, so an existing database may also
contain legacy or custom types outside the UI matrix. `post_statuses` compares
the backend list with the SQL constraint and reports missing customer labels.

`api.registered_routes` includes only Actix attribute routes registered in
`startup::configure`; the OpenAPI difference lists expose documentation drift.
`capabilities[*].status` is `AVAILABLE` when all required routes are registered
and `MISSING_CAPABILITY` otherwise. Availability is only a signal to *run* a
behavior test. It never asserts authorization, state transitions, UI rendering,
or audit correctness. Those require independent observers and live scenarios.

The scanner intentionally uses Python standard library only. It does not read
production credentials or mutate a database. A live DB observer should add
data-specific board types and state checks when a disposable test database is
available.
