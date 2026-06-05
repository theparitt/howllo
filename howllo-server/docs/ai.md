# AI Assistance

AI is **advisory only** — it suggests, humans decide.

## Configuration

```bash
AI_ENABLED=true
```

Default: AI is **disabled**. Set `AI_ENABLED=true` to enable.

V1 provides a local heuristic AI for development and testing. External AI provider support (OpenAI, etc.) is planned for a future release.

## Suggestion Types

| Type | Endpoint | Description |
|------|----------|-------------|
| Duplicate | `POST /api/admin/ai/posts/{id}/duplicate-suggestions` | Similar post detection |
| Tags | `POST /api/admin/ai/posts/{id}/tag-suggestions` | Tag recommendations |
| Summary | `POST /api/admin/ai/posts/{id}/summary` | Thread summary |
| Moderation | `POST /api/admin/ai/posts/{id}/moderation-suggestion` | Moderation flags |
| Grouping | `POST /api/admin/ai/posts/{id}/grouping-suggestion` | Roadmap grouping |

## Review Workflow

1. Admin requests AI suggestion
2. AI creates a reviewable suggestion (stored in `ai_suggestions` table)
3. Admin reviews the suggestion
4. Admin accepts or rejects: `PATCH /api/admin/ai/suggestions/{id}` with `{"status": "accepted"|"rejected"}`
5. Accepted suggestions create audit log entries

## Safety Rules

AI **cannot**:
- Delete posts
- Hide posts
- Change post status
- Mark duplicates directly
- Publish roadmap changes
- Bypass any permission check

AI only inserts rows into the `ai_suggestions` table. The `ai/mod.rs` file contains zero `UPDATE posts` or `DELETE FROM posts` queries.

## Provider

V1 supports local heuristic AI. The `AiAssistant` trait allows future provider additions.

## Mock Provider

Tests use the local provider. No external API calls during testing.
