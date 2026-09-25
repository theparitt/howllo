# Howllo Server

Howllo Server is a multi-tenant feedback management backend for collecting, moderating, and publishing user feedback.

It supports local board accounts and generic OpenID Connect without RooIAM. RooIAM is an optional integration. See [authentication setup](../docs/auth/README.md).

## Features

- **Boards** — Public and private feedback boards per tenant
- **Posts** — Create, edit, vote, comment, and follow feedback posts
- **Roadmap** — Public roadmap with status tracking (planned → in_progress → done)
- **Moderation** — Hide posts/comments, soft-delete, mark duplicates, lock posts
- **Tags** — Organize feedback with admin-managed tags
- **Audit Log** — Every admin/moderator action is traceable with request IDs
- **Webhooks** — Outbound HTTP delivery with HMAC SHA-256 signatures
- **API Tokens** — Scoped read tokens for integrations
- **Search** — PostgreSQL full-text search with relevance ranking
- **AI Suggestions** — Advisory-only AI for duplicates, tags, summaries (human must approve)
- **Notifications** — In-app follower notifications for status changes and official responses
- **Realtime** — WebSocket events for live updates

## Architecture

```
HTTP Handler → Service → Repository → PostgreSQL
  (parse)       (logic)    (SQL only)   (storage)
```

- **Handlers** — Parse HTTP requests, call services, return responses
- **Services** — Permission checks, business validation, workflow, transactions, audit
- **Repositories** — SQL queries only, no business logic
- **13 services, 15 repositories, 89 tests, 12 migrations**

## Quickstart

```bash
cp .env.example .env
# Edit .env with your PostgreSQL connection
docker compose up --build
curl http://localhost:7700/api/health
curl http://localhost:7700/api/ready
```

## Documentation

- [Quickstart](docs/quickstart.md)
- [Configuration](docs/configuration.md)
- [Database](docs/database.md)
- [API Reference](docs/api.md)
- [Permissions](docs/permissions.md)
- [Moderation](docs/moderation.md)
- [Roadmap](docs/roadmap.md)
- [Webhooks](docs/webhooks.md)
- [API Tokens](docs/api-tokens.md)
- [Search](docs/search.md)
- [AI](docs/ai.md)
- [Security](docs/security.md)
- [Authentication providers](../docs/auth/README.md)
- [Self-hosting](docs/self-hosting.md)
- [Backup/Restore](docs/backup-restore.md)

## OpenAPI

Full API specification at [openapi.yaml](openapi.yaml) (732 lines, all 50+ endpoints documented).

## License

MIT
