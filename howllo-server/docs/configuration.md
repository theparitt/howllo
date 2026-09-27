# Configuration

All configuration is via environment variables. Create a `.env` file from `.env.example`.

## Required

| Variable | Description | Example |
|----------|-------------|---------|
| `HOWLLO_DATABASE_URL` (or `DATABASE_URL`) | PostgreSQL connection string | `postgres://postgres:password@localhost:5432/howllo` |
| `HOWLLO_BIND_ADDRESS` | Server listen address | `127.0.0.1:7700` |
| `HOWLLO_JWT_SECRET` | Legacy RooIAM JWT secret, only when enabled | Use a generated secret |

## Security

| Variable | Description | Default |
|----------|-------------|---------|
| `HOWLLO_ALLOWED_ORIGINS` | CORS allowed origins (comma-separated) | `http://localhost:3000` |
| `HOWLLO_RATE_LIMIT_ENABLED` | Enable PostgreSQL-backed public write rate limiting | `true` |
| `HOWLLO_PUBLIC_WRITE_RATE_LIMIT` | Maximum writes per client IP per minute for each write group | `60` |
| `HOWLLO_TRUSTED_PROXY_CIDRS` | Comma-separated proxy IP ranges allowed to supply the client IP | `127.0.0.0/8,::1/128` |
| `HOWLLO_CLIENT_IP_HEADER` | Header supplied by the trusted proxy: `cf-connecting-ip` or `x-real-ip` | `cf-connecting-ip` |

If using Nginx, configure it to **overwrite** `X-Real-IP` with the real client
address, set `HOWLLO_CLIENT_IP_HEADER=x-real-ip`, and list only the proxy's
network in `HOWLLO_TRUSTED_PROXY_CIDRS`. Never trust a public client subnet.
Direct client headers are ignored. If the proxy is not trusted, the peer IP is
used instead; this may rate-limit all customers behind one proxy.

## Input Limits

| Variable | Description | Default |
|----------|-------------|---------|
| (fixed) | Post title max length: 255 characters | 255 |
| `HOWLLO_MAX_POST_BODY_CHARS` | Max post body length | `10000` |
| `HOWLLO_MAX_COMMENT_BODY_CHARS` | Max comment body length | `4000` |

## Webhook

| Variable | Description | Default |
|----------|-------------|---------|
| `HOWLLO_WEBHOOK_TIMEOUT_MS` | Webhook delivery timeout (milliseconds) | `5000` |

## AI (Disabled by default)

| Variable | Description | Default |
|----------|-------------|---------|
| `AI_ENABLED` | Enable AI suggestions | `false` |
| `AI_BASE_URL` | AI provider base URL | (none) |
| `AI_MODEL` | AI model name | `gemma4` |

AI provider: V1 supports local heuristic AI only. External AI provider integration is planned.
