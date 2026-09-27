# Self-Hosting

## Prerequisites

- Linux or macOS
- Rust 1.80+ with `cargo`
- PostgreSQL 16+
- Docker (optional)

## Setup

```bash
git clone https://github.com/theparitt/howllo
cd howllo/howllo-server
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

This Compose file runs only the API and PostgreSQL with development secrets.
It is an API build example, not the complete self-hosted deployment. The App,
Web, Admin, TLS proxy, MinIO and production secret setup are still being
assembled into a reference stack. See [the release roadmap](../../docs/roadmap.md).

## Verify

```bash
curl http://localhost:5110/api/health
# {"status":"ok"}

curl http://localhost:5110/api/ready
# {"status":"ready"}
```

## Production Considerations

- Run behind a reverse proxy (nginx, Caddy)
- Set `HOWLLO_ALLOWED_ORIGINS` to your frontend domain
- Keep rate limiting enabled: `HOWLLO_RATE_LIMIT_ENABLED=true`
- Set `HOWLLO_TRUSTED_PROXY_CIDRS` to the address range of your reverse proxy;
  see [configuration.md](configuration.md) for the required client IP header.
- Use strong authentication secrets for the providers you enable.
- Configure `HOWLLO_WEBHOOK_TIMEOUT_MS` for webhook delivery
