# RooIAM integration

RooIAM is optional. Howllo runs with no RooIAM account, SDK or cloud credentials.

The hosted `https://api.rooiam.com` discovery document currently advertises HS256 ID tokens. Howllo's generic OIDC adapter accepts RS256, so **use the RooIAM widget and hosted userinfo integration for this hosted service**. If a RooIAM installation later publishes RS256 ID tokens, it can use the generic OIDC setup below.

For an RS256 RooIAM installation: create an OIDC client, register `https://YOUR_API/api/auth/callback/rooiam`, then add an entry to `HOWLLO_OIDC_PROVIDERS`. The essential values are the exact OIDC issuer and client ID; add a client secret for a confidential client, plus `token_endpoint_auth_method` if its token endpoint requires a method other than `client_secret_basic`.

Example:

```env
HOWLLO_PUBLIC_API_URL=https://api.howllo.example
HOWLLO_WEB_ORIGIN=https://app.howllo.example
HOWLLO_OIDC_PROVIDERS=[{"id":"rooiam","display_name":"RooIAM","issuer":"https://YOUR_RS256_ROOIAM_ISSUER","client_id":"YOUR_CLIENT_ID","scopes":"openid profile email"}]
```

For the Howllo App widget deployment, the server needs `HOWLLO_ROOIAM_WIDGET_BASE_URL`, `HOWLLO_ROOIAM_WORKSPACE_ID`, and `HOWLLO_ROOIAM_CLIENT_ID`. Howllo App needs the corresponding three `NEXT_PUBLIC_ROOIAM_WIDGET_*` values for account login before a workspace is selected. Set the workspace authentication setting to RooIAM where the widget is used. Register the App callback (`http://localhost:7702/auth/callback` locally) on that RooIAM client. Set `HOWLLO_ROOIAM_HOSTED_USERINFO_URL` to the trusted RooIAM userinfo endpoint for the bearer-token exchange. Set `HOWLLO_ROOIAM_LEGACY_HS256_ENABLED=true` only if a legacy installation explicitly shares an HS256 key with RooIAM; it defaults to false.

For the current split deployment, keep `HOWLLO_AUTH_LOCAL_ENABLED=true` for Howllo Web's public accounts, set `HOWLLO_OIDC_PROVIDERS=[]` and `HOWLLO_WORKSPACE_AUTH_PROVIDER=rooiam` on the server, and set `NEXT_PUBLIC_HOWLLO_AUTH_PROVIDERS=rooiam` in Howllo App. Keep `HOWLLO_ROOIAM_LEGACY_HS256_ENABLED=false` when hosted userinfo is configured. Register the exact App origin's `/auth/callback` URL and allow that origin as an embed origin in the RooIAM client. Existing workspaces with an explicit local auth row must be changed to RooIAM; new workspaces inherit the env default. Howllo Web uses `NEXT_PUBLIC_HOWLLO_AUTH_PROVIDERS=local` and has no RooIAM widget configuration.

Existing RooIAM users are mapped to `provider_id='rooiam'` using their old subject, preserving their Howllo user IDs and workspace permissions.
