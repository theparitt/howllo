# Howllo Server Roadmap

## ✅ Completed (V1)

- [x] Multi-tenant feedback boards
- [x] Post CRUD with pagination, filtering, sorting
- [x] Comments, votes, follows
- [x] Tag management
- [x] Status transitions with guardrails
- [x] Duplicate detection and workflow
- [x] Role/permission model (owner/admin/moderator/member)
- [x] Private board enforcement
- [x] Soft-delete and restore
- [x] Locked post behavior
- [x] Moderation queue
- [x] Post activity feed
- [x] Board and tag summaries
- [x] Public roadmap with status grouping
- [x] Audit log with request_id tracking
- [x] Webhook delivery with HMAC signatures and retry
- [x] API tokens with scopes, expiry, and last_used_at
- [x] PostgreSQL full-text search (tsvector + GIN) with relevance ranking
- [x] AI suggestion system (advisory only, local heuristic)
- [x] In-app notifications
- [x] Realtime WebSocket events
- [x] Rate limiting
- [x] CORS configuration
- [x] Docker and CI/CD (12 migrations, 89 tests)
- [x] OpenAPI specification (732 lines, 50+ endpoints)
- [x] 16-page documentation suite

## 🚧 In Progress

- [ ] External AI provider integration (OpenAI, etc.)
- [ ] Webhook retry worker with exponential backoff
- [ ] Redis-backed rate limiting for multi-instance

## 📋 Planned (V2)

- [ ] Webhook retry with exponential backoff
- [ ] Multi-instance rate limiting (Redis)
- [ ] Email notifications
- [ ] Slash command integration
- [ ] Rich text post body
- [ ] Image attachments
- [ ] Keyboard shortcut navigation

## 📋 Future

- [ ] Federated feedback (cross-tenant)
- [ ] Custom status workflows
- [ ] Advanced analytics dashboard
- [ ] Public voting without account
