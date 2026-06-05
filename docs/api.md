# API

REST-first JSON API served by `howllo-server` on port 5110. All routes are
tenant-aware and authorized server-side.

## Route groups

All routes are prefixed `/api` (not `/v1`).

```text
/api/boards                       board list + detail
/api/boards/{slug}/posts          posts in a board (GET list, POST create)
/api/posts/{id}                   post detail / update; comments, votes, follow nested under it
/api/posts/{id}/comments          comments
/api/posts/{id}/vote              POST/DELETE vote
/api/posts/{id}/follow            POST/GET/DELETE follow state
/api/roadmap, /roadmap/by-status, /roadmap/by-tag
/api/tags                         tags
/api/notifications                in-app notifications (list, mark read)
/api/admin/*                      moderation, status (PATCH), official responses,
                                  boards/tags CRUD, webhooks, api-tokens, exports
/api/health                       liveness
/ws                               realtime websocket (see Realtime below)
```

## Conventions

- Tenant is resolved per request (slug query/header), then authorized.
- Admin moderation endpoints use **PATCH** (status, lock, visibility, duplicate,
  soft-delete), authorized via tenant membership role server-side.
- Status transitions are admin-controlled and validated against the status
  state machine (`under-review → planned → in-progress → done | declined`).
- Errors map to predictable HTTP codes with a `{ code, message }` body. Internal
  DB errors are never leaked to clients.

## Realtime (websocket)

`/ws` (public URL proxied on port 5113) carries live events. It is **additive**:
the same events that write a notification row and emit a webhook also broadcast
on the socket. Clients treat WS as a "ping to refetch" — the REST API and the
`notifications` table remain the source of truth. Sockets authenticate with a
short-lived ticket and only ever receive their own tenant's events.

The typed client lives in [`shared/api-client`](../shared/api-client).
Keep [`shared/types`](../shared/types) in sync with server DTOs.
