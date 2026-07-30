# End-user SSO (token-exchange)

Let a workspace's **end-users** post/vote without any howllo/rooiam login — their
identity comes from the customer's own product. The customer's backend signs a
short-lived JWT with the workspace's SSO secret; howllo verifies it and mints a
session. This is the white-label / embedded path (the same model as Canny).

Staff (owner/admin/moderator) still sign in with rooiam. Only end-user identity
is delegated. See docs/ACTIVITY_AND_INVITATIONS.md + the auth notes in memory.

## Setup (owner)
Manage → **Sign-in (SSO)** → **Enable SSO**. You get a per-workspace secret
(`sk_…`) and a copy-paste snippet. Keep the secret on your server only — it is
never returned by the public `/api/workspace-auth` endpoint, only to
owners/admins via `GET /api/admin/sso-config`.

## Flow
```
customer app (user already logged in)
      │  sign JWT { sub, email, name, exp }  with the workspace secret (HS256)
      ▼
POST {API}/api/auth/sso-session   { tenant_slug, token }
      │  howllo verifies signature with the workspace's sso_secret
      ▼
{ session_token }   ->  use as Bearer for the board (howllo_ws_…, 30d)
```

Identity is namespaced per workspace as `sso:<tenant_id>:<sub>`, so a customer's
`sub` can never collide with another customer's or with a rooiam user.

## Integration (Node)
```js
import jwt from "jsonwebtoken";
const token = jwt.sign(
  { sub: user.id, email: user.email, name: user.name },
  process.env.HOWLLO_SSO_SECRET,          // the workspace secret
  { algorithm: "HS256", expiresIn: "1h" },
);
const { session_token } = await (await fetch(`${API}/api/auth/sso-session`, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ tenant_slug: "<your-workspace-slug>", token }),
})).json();
```

## Endpoints
- `POST /api/auth/sso-session` `{tenant_slug, token}` → `{session_token}` (public;
  rejects with 401 on a bad signature, 422 if SSO is disabled for the workspace).
- `GET  /api/admin/sso-config?tenant_slug=` → `{enabled, secret, session_path}` (ManageSettings).
- `POST /api/admin/sso-config/regenerate?tenant_slug=` → enable / rotate the secret.
- `DELETE /api/admin/sso-config?tenant_slug=` → disable (existing sessions keep working).

## Notes
- Secret rotation invalidates tokens signed with the old secret immediately;
  already-minted `howllo_ws_` sessions keep working until they expire/are revoked.
- Tokens should be short-lived (`exp`); howllo enforces `exp`.
- `email`/`name` are optional; a missing email becomes `<sub>@sso.local`.
