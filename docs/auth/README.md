# Howllo authentication

Howllo does not require RooIAM. A fresh installation supports local board accounts. Administrators can enable any number of OpenID Connect providers. RooIAM is an optional login widget integration; a RooIAM issuer that signs RS256 ID tokens can also use generic OIDC. The hosted RooIAM issuer currently advertises HS256, so it uses the widget integration.

```
browser → /api/auth/providers → local or OIDC → Howllo account session
                                           → workspace session → board permissions
```

`users.id` is Howllo's permanent user ID. `user_identities(provider_id, subject)` maps provider identities to that ID. Email is profile data, never an automatic account-linking key. Existing RooIAM users are backfilled without changing their IDs, memberships, posts or votes. Explicit account linking is not implemented.

Signing in to a public board creates a public-participant membership. It permits public board activity but does not open private boards. A workspace manager must explicitly grant private access through a managed membership or account role.

Provider configuration is global in `HOWLLO_OIDC_PROVIDERS`. Workspace membership and roles remain in Howllo. Local accounts use usernames and Argon2id passwords, with one-time recovery codes for forgotten passwords. The admin console's own bootstrap password remains a separate system administrator credential.

## Select providers in `.env`

- **Local only (default):** `HOWLLO_AUTH_LOCAL_ENABLED=true` and `HOWLLO_OIDC_PROVIDERS=[]`.
- **OIDC only:** set `HOWLLO_AUTH_LOCAL_ENABLED=false` and put at least one enabled object in `HOWLLO_OIDC_PROVIDERS`.
- **Multiple sign-in choices:** leave local enabled and put multiple objects in the one JSON array. An object with `enabled=false` is hidden and cannot sign in.
- **Current split deployment:** set `HOWLLO_AUTH_LOCAL_ENABLED=true`, `HOWLLO_WORKSPACE_AUTH_PROVIDER=rooiam`, and `HOWLLO_OIDC_PROVIDERS=[]`. Set the RooIAM widget/userinfo values on the server and `NEXT_PUBLIC_ROOIAM_WIDGET_*` in `howllo-app/.env.local`. Set `NEXT_PUBLIC_HOWLLO_AUTH_PROVIDERS=local` in `howllo-web/.env.local`. Existing workspaces with an explicit `local` workspace auth row need their setting changed to RooIAM for management sign-in. Local accounts remain available for public board participants.

Each OIDC object needs `id`, `display_name`, the provider's exact `issuer`, `client_id`, and `scopes` including `openid`. Add `client_secret` and, when required, `token_endpoint_auth_method`. Set `HOWLLO_PUBLIC_API_URL` and `HOWLLO_WEB_ORIGIN` to browser-reachable origins. Register `{HOWLLO_PUBLIC_API_URL}/api/auth/callback/{id}` with each provider. The commented configurations for RooIAM, Google, Microsoft, Keycloak and Authentik are in [the server env example](../../howllo-server/.env.example). This is a platform-wide default; tenant-owned customer providers are configured in the workspace app.

## Customer sign-in per workspace

Staff sign-in, platform admin sign-in, and customer sign-in are separate. The **Customer sign-in** page in each workspace lets an owner/admin keep local passwords and recovery codes, optionally enable RooIAM, and add Google, Microsoft Entra, or a generic OpenID Connect provider using that tenant's own Client ID and Client Secret. The page shows the exact redirect URI to register with each provider. Microsoft requires a tenant-specific Entra directory ID in the issuer URL. Generic OIDC issuers must appear in the server's `HOWLLO_OIDC_ALLOWED_ISSUERS` list.

Set `HOWLLO_OIDC_CONFIG_KEY` to one stable 32-byte hex key before storing any tenant OIDC secret. Secrets are encrypted in Postgres and are never returned by the API. Back up this key with the database; rotating it requires re-entering provider secrets. Set `HOWLLO_CUSTOMER_WEB_ORIGIN` to the public board site's origin so customer OIDC callbacks return to Howllo Web. The server exposes only enabled sign-in choices for the selected workspace. An OIDC account token is scoped to that workspace; matching email addresses do not merge accounts. Existing workspace sessions stay valid when a sign-in method is disabled; revoke sessions separately if immediate sign-out is required.

For customer RooIAM, register the public board site's `/auth/callback` redirect URI with the tenant's RooIAM client. Howllo checks token introspection against that workspace's configured client ID before issuing a customer workspace session. Staff RooIAM configuration remains separate.

The older RooIAM widget uses three different values: `HOWLLO_ROOIAM_WIDGET_BASE_URL`, `HOWLLO_ROOIAM_WORKSPACE_ID`, `HOWLLO_ROOIAM_CLIENT_ID`; account-level login also needs the matching `NEXT_PUBLIC_ROOIAM_WIDGET_*` values in Howllo App. Those are widget compatibility settings and are not needed for RooIAM OIDC. Never put an OIDC client secret in a `NEXT_PUBLIC_*` variable.

Guides: [local](local.md), [OIDC](oidc.md), [RooIAM](rooiam.md), [Google](google.md), [Microsoft Entra ID](microsoft.md), [Keycloak](keycloak.md), [Authentik](authentik.md), [troubleshooting](troubleshooting.md).

Supported login modes are local, generic OIDC, the RooIAM widget, Google, Microsoft Entra ID, Keycloak and Authentik. Google, Microsoft, Keycloak and Authentik use the generic OIDC adapter. No provider account is required to boot Howllo.

The compatibility RooIAM widget and `/api/auth/workspace-session` RooIAM token exchange remain available for existing installations. New integrations should use OIDC. The legacy widget needs its own callback registered at the RooIAM client.

## Deployment shapes

| Setup | Services | Howllo configuration |
| --- | --- | --- |
| Howllo only | Howllo + PostgreSQL (and local storage or MinIO) | Local accounts enabled; no external IdP |
| Company SSO | Company OIDC → Howllo → PostgreSQL | One `HOWLLO_OIDC_PROVIDERS` entry |
| RooIAM hosted | RooIAM widget → Howllo → PostgreSQL | Widget/client/userinfo values; local may be disabled |
| Multiple providers | Google, Microsoft, company OIDC and RooIAM → Howllo | Multiple entries; each has its own stable provider ID |

All modes use the same Howllo user, workspace, board and permission model. The initial local setup needs no RooIAM account, API key or RooIAM Cloud connection.
