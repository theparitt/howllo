# Contributing

## Setup

```bash
cp .env.example .env
# Edit .env with your Postgres settings

DATABASE_URL=postgres://postgres:password@localhost:5432/howllo_test
cargo install sqlx-cli --no-default-features --features rustls,postgres

# Create test database
psql -U postgres -c "CREATE DATABASE howllo_test;"

# Run migrations
DATABASE_URL=postgres://postgres:password@localhost:5432/howllo_test sqlx migrate run --source db/migrations

# Run tests
DATABASE_URL=postgres://postgres:password@localhost:5432/howllo_test cargo test
```

## Quality Gates

Before submitting a PR:
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo sqlx prepare --check -- --all-targets
```

## Code Style

- Handlers: parse request → call service → return response
- Services: validation → permission → transaction → audit → notify
- Repositories: SQL only
- Never write SQL in handlers or services
- All admin/moderator mutations must write audit log

## Testing

- Add tests for every new endpoint, permission boundary, and edge case
- Test cross-tenant isolation
- Test public/private/hidden/deleted content boundaries
- Use `seed_basic_tenant()` for test setup

## Architecture Rules

1. Handlers are thin
2. Services own business rules
3. Repositories own SQL
4. AI is advisory only — never mutates data directly
5. API tokens are read-only
6. Audit log on every admin/moderator mutation
