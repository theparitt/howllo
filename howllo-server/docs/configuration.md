# Configuration

All configuration is via environment variables. Create a `.env` file from `.env.example`.

## Required

| Variable | Description | Example |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | `postgres://postgres:password@localhost:5432/howllo` |
| `BIND_ADDRESS` | Server listen address | `0.0.0.0:5110` |
| `ROOIAM_JWT_SECRET` | JWT signing secret for user auth | `your-secret-here` |

## Security

| Variable | Description | Default |
|----------|-------------|---------|
| `HOWLLO_ALLOWED_ORIGINS` | CORS allowed origins (comma-separated) | `http://localhost:3000` |
| `HOWLLO_RATE_LIMIT_ENABLED` | Enable rate limiting | `false` |
| `HOWLLO_PUBLIC_WRITE_RATE_LIMIT` | Max public writes per window | `60` |

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
