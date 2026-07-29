# Activity, Notifications & Invitations — Design

How Howllo keeps people in the loop: workspace **staff invitations**, a durable
**notification** history, real-time **toasts**, and an **activity feed** that is
the home page for a signed-in user.

Design goals: one source of truth, reuse what already exists (`notifications`,
`post_follows`, the realtime `Hub`), and make the surfacing feel *smooth* —
optimistic UI, real-time deltas, no full-page reloads.

---

## 0. What already exists (build on, don't duplicate)

- `notifications(id, tenant_id, user_id, post_id, event_type, title, body, is_read, created_at)`
  indexed by `(user_id, created_at DESC)`.
- `GET /api/notifications`, `PATCH /api/notifications/{id}/read`.
- `post_follows` with `notify_on_status_change`, `notify_on_official_response`.
- Fan-out on **status change** and **official response** → `create_follow_notifications`.
- Realtime `Hub`: WS channels keyed by `tenant_id`; the handler already resolves
  the connected `user_id` and checks membership before subscribing.
- Members are invited by email today via a hacky `invited:<email>` placeholder
  user. **This design replaces that** with a real invitations table.

Gaps this design fills: invitation lifecycle, comment/reply notifications,
auto-follow so "my posts / posts I commented on" light up, per-user realtime
delivery, unread counts, and the whole web surface (center, toasts, feed).

---

## 1. Workspace Staff Invitations

A tenant owns many workspaces; each workspace has staff with roles
(`owner | admin | moderator | member`). Owners/admins invite staff **by email**;
the invitee explicitly **accepts / rejects / skips**; the inviter can **withdraw**.
Everything emits a notification and is auditable.

### 1.1 Data model — `workspace_invitations`

| column           | type        | notes |
|------------------|-------------|-------|
| id               | UUID PK     | |
| tenant_id        | UUID FK     | the workspace |
| email            | TEXT        | lowercased; who is invited |
| role             | TEXT        | `admin \| moderator \| member` (not `owner`) |
| status           | TEXT        | `pending \| accepted \| rejected \| withdrawn \| expired` |
| invited_by       | UUID FK     | actor user |
| user_id          | UUID FK?    | filled once a matching account signs in (nullable) |
| created_at       | TIMESTAMPTZ | |
| responded_at     | TIMESTAMPTZ | set on accept/reject/withdraw |
| expires_at       | TIMESTAMPTZ | optional (e.g. +14 days) |

- Partial unique index: **one `pending` invite per `(tenant_id, email)`**. A new
  invite after reject/withdraw is allowed (history is kept).
- `member` invites are allowed but optional — plain sign-in already grants
  `member`. Invites matter for `admin`/`moderator`.

### 1.2 Lifecycle

```
owner creates ──▶ pending ──▶ accept  ──▶ accepted   → membership(role) created
                     │        reject  ──▶ rejected
                     │        skip     ─── (stays pending, just dismissed in UI)
                     └── owner withdraw ▶ withdrawn
                     └── time passes    ▶ expired
```

- **Accept** → upsert `memberships(tenant, user, role)` (override, but never
  demote an existing owner) → invite `accepted`.
- **Reject** → invite `rejected`; no membership.
- **Skip** → no state change; the invitee's client just hides it this session
  (a `skipped_at` per-user dismissal is optional; keep it client-side first).
- **Withdraw** (owner) → `withdrawn`; if a membership was already created by a
  prior accept, withdrawing does **not** silently strip it — that's a separate
  "remove member" action (keeps the two verbs honest).

### 1.3 Binding email → account at sign-in

`create_workspace_session` already upserts the real rooiam user. Add: after
upsert, if a `pending` invite exists for `(tenant, lower(email))` with
`user_id IS NULL`, stamp `user_id` = real user. The invite then shows up in the
invitee's "pending invitations" so they can act on it. **We do not auto-accept**
— the user chooses. (Plain `member` grant on sign-in stays as-is.)

### 1.4 APIs

Owner/admin (require `ManageMembers`):
- `POST   /api/admin/invitations` `{tenant_slug, email, role}` → create
- `GET    /api/admin/invitations?tenant_slug=` → list (all statuses, for history)
- `POST   /api/admin/invitations/{id}/withdraw`

Invitee (workspace session):
- `GET    /api/me/invitations` → my pending invites across workspaces
- `POST   /api/me/invitations/{id}/accept`
- `POST   /api/me/invitations/{id}/reject`
  (skip is client-only)

Each transition writes an **audit log** row and a **notification** (§2.2).

---

## 2. Notifications engine (extend the existing one)

### 2.1 Subscriptions = follows (reuse, extend)

Make "my posts" and "posts I commented on" light up for free by **auto-following**:

- Author **auto-follows** their post on create (all notify flags on).
- Commenter **auto-follows** the post on first comment.
- Add column `post_follows.notify_on_comment BOOLEAN DEFAULT TRUE`.

Then the *existing* follow fan-out naturally covers authored + participated +
explicitly-followed posts. Users can still mute per post/flag.

### 2.2 Event catalog

| event_type              | recipients                                   | trigger |
|-------------------------|----------------------------------------------|---------|
| `post_comment`          | post followers (author auto-follows)         | new comment on a post |
| `comment_reply`         | parent-comment author                        | reply to a comment |
| `status_changed` ✓      | followers w/ `notify_on_status_change`       | admin status change |
| `official_response` ✓   | followers w/ `notify_on_official_response`   | official reply |
| `post_vote_milestone`   | post author                                  | vote crosses 10/25/50… (throttled) |
| `invitation_received`   | invited user                                 | owner creates invite |
| `invitation_accepted`   | inviter                                      | invitee accepts |
| `invitation_rejected`   | inviter                                      | invitee rejects |
| `invitation_withdrawn`  | invited user                                 | owner withdraws |

Rules: **never notify the actor about their own action**; collapse duplicates
(e.g. 3 comments in a minute → keep rows but the feed groups them, §4).

### 2.3 Per-user realtime delivery

Add `target_user_id: Option<Uuid>` to `RealtimeEvent`. The WS handler already
knows the connected `user_id`; forward an event only when `target_user_id` is
`None` (broadcast) **or** equals the socket's user. On each notification write,
`hub.broadcast(tenant, RealtimeEvent{ type:"notification", target_user_id:Some(u), … })`.

### 2.4 API additions

- `GET  /api/notifications?unread=1&cursor=` → paginated history
- `GET  /api/notifications/unread-count` → badge number
- `POST /api/notifications/read-all`
- (keep) `PATCH /api/notifications/{id}/read`

---

## 3. Web — notification center + toasts (smooth)

One provider owns notification state; the center and toasts are two views of it.

- **`NotificationProvider`** (client): loads recent notifications + unread count,
  opens the WS, and on a `notification` event **prepends optimistically** and
  bumps the badge. Single source of truth.
- **Bell + center** in the header: unread badge, dropdown list (grouped, §4),
  "mark all read", link to the full feed.
- **Toast stack** (bottom-right): derived from the *diff* of the notification
  list — when a new item arrives via WS, show a toast (title + one-line body +
  action link), auto-dismiss ~5s, max 3 stacked + "+N more", pausable on hover.
  Respects `prefers-reduced-motion`. (Mirrors the pattern proven in araihub.)
- **Invitation toasts/cards** are special: they carry **Accept / Reject / Skip**
  buttons inline (optimistic; calls §1.4), so staff can act without leaving the page.

Smoothness: optimistic add + spring-in, WS-driven (no polling), graceful
reconnect/backoff (reuse `lib/realtime.ts`), skeletons on first load.

---

## 4. Web — Activity Feed (the signed-in home page)

When signed in, `/` (or `/{tenant}`) shows the **feed** instead of the board grid:
a reverse-chronological, grouped timeline built from the user's notifications.

- **Grouping**: collapse by `(post_id, event_type)` within a short window —
  "Alice and 3 others commented on *Dark mode*", "*Slack notifications* moved to
  In progress". Each group links to the post/thread.
- **Sections/filter chips**: `All · Following · My posts · Mentioned/Replied ·
  Invitations`. "My posts" and "Following" fall out of the auto-follow model.
- **Pending invitations** pinned at the top as actionable cards.
- Empty state → nudge to follow boards/posts.
- Reads straight from the notifications history API (feed = rendered
  notifications; no second store).

---

## 5. Build order (staged, each shippable)

1. **Invitations backend** — table + owner/invitee APIs + accept→membership +
   sign-in binding + audit + invite notifications. Retire the `invited:<email>`
   placeholder. *(tests: create→accept→member; reject; withdraw; no owner demote)*
2. **Notifications engine** — auto-follow (author/commenter), `notify_on_comment`,
   `post_comment`/`comment_reply` fan-out, `target_user_id` realtime, unread-count
   + read-all APIs.
3. **Web notifications** — `NotificationProvider`, header bell + center, toast
   stack (incl. invitation action cards).
4. **Web activity feed** — grouped timeline home page + filter chips + pinned
   invites.

Each stage builds + is verified live before the next. Admin (howllo-admin) gets
an **Invitations** panel in the Members page during stage 1.

---

## 6. Open decisions (pick before/with stage 1)

- **Invite as `member`?** Allowed but low-value (sign-in already grants member).
  Default: allow, but UI emphasizes admin/moderator.
- **Expiry**: on (14d) or off for now? Default: column exists, enforcement later.
- **Skip persistence**: client-only vs `skipped_at`. Default: client-only first.
- **Feed vs board on `/`**: feed for signed-in, board grid for anonymous.
