# Self-Hosting

## Prerequisites

- Linux or macOS
- Rust 1.80+ with `cargo`
- PostgreSQL 16+
- Docker (optional)

## Setup

```bash
git clone https://github.com/howllo/howllo-server
cd howllo-server
cp .env.example .env
# Edit .env with your settings
```

## Configuration

See [configuration.md](configuration.md) for all environment variables.

## Database

```bash
psql -U postgres -c "CREATE DATABASE howllo;"
DATABASE_URL=postgres://postgres:password@localhost:5432/howllo sqlx migrate run --source db/migrations
```

## Running

### Cargo
```bash
DATABASE_URL=postgres://postgres:password@localhost:5432/howllo cargo run --release
```

### Docker
```bash
docker compose up --build
```

## Verify

```bash
curl http://localhost:5110/api/health
# {"status":"ok"}

curl http://localhost:5110/api/ready
# {"status":"ready","database":"connected"}
```

## Production Considerations

- Run behind a reverse proxy (nginx, Caddy)
- Set `HOWLLO_ALLOWED_ORIGINS` to your frontend domain
- Enable rate limiting: `HOWLLO_RATE_LIMIT_ENABLED=true`
- Use a strong `ROOIAM_JWT_SECRET`
- Configure `HOWLLO_WEBHOOK_TIMEOUT_MS` for webhook delivery
