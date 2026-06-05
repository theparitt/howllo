# Quickstart

## Prerequisites

- Rust 1.80+
- PostgreSQL 16+
- Docker (optional)

## Local Development

```bash
# Clone and set up
cp .env.example .env
# Edit .env with your PostgreSQL connection string

# Create database
psql -U postgres -c "CREATE DATABASE howllo;"

# Run migrations
DATABASE_URL=postgres://postgres:password@localhost:5432/howllo sqlx migrate run --source db/migrations

# Start server
cargo run

# Verify
curl http://localhost:5110/api/health
curl http://localhost:5110/api/ready
```

## Docker

```bash
# Start PostgreSQL + server
docker compose up --build

# Test with test database
docker compose -f docker-compose.test.yml up -d
```

## Running Tests

```bash
# Create test database
psql -U postgres -c "CREATE DATABASE howllo_test;"

# Run migrations on test DB
DATABASE_URL=postgres://postgres:password@localhost:5432/howllo_test sqlx migrate run --source db/migrations

# Run all tests
DATABASE_URL=postgres://postgres:password@localhost:5432/howllo_test cargo test

# Quality checks
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo sqlx prepare --check -- --all-targets
```

## First API Calls

Create a tenant (requires direct DB or seed script), then:

```bash
# List public boards
curl "http://localhost:5110/api/boards?tenant_slug=acme"

# View roadmap
curl "http://localhost:5110/api/roadmap?tenant_slug=acme"

# Search posts
curl "http://localhost:5110/api/search/posts?tenant_slug=acme&q=dark+mode"
```
