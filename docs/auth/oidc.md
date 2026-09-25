# Generic OpenID Connect

Set the API and web origins to browser-reachable values:

```env
HOWLLO_PUBLIC_API_URL=https://api.example.com
HOWLLO_WEB_ORIGIN=https://app.example.com
HOWLLO_AUTH_LOCAL_ENABLED=true
HOWLLO_OIDC_PROVIDERS=[{"id":"company","display_name":"Company SSO","issuer":"https://id.example.com","client_id":"howllo","client_secret":"replace-me","scopes":"openid profile email","enabled":true}]
```

Register `https://api.example.com/api/auth/callback/company` as an allowed redirect URI at the provider. The ID suffix must match the configured provider `id`. The discovery document must be available at `{issuer}/.well-known/openid-configuration`; its issuer must exactly match the configured issuer. Howllo currently accepts RS256 signed ID tokens and S256 PKCE. `client_secret` may be omitted when the provider supports public clients with PKCE and `token_endpoint_auth_method=none`. Use HTTPS except for localhost development.

The backend handles discovery, authorization code exchange, state, nonce, PKCE, ID token signature, issuer, audience, expiry and issued-at checks. State is also bound to an HTTP-only SameSite cookie. JWKS is fetched on every callback, including after key rotation. A one-time exchange code brings the Howllo account session back to the web app. Secrets and provider tokens are not sent to the browser.

`token_endpoint_auth_method` supports `client_secret_basic` (default when a secret is set), `client_secret_post`, or `none` (default without a secret). Choose the method required by the provider's client registration. For `client_secret_post`, Howllo sends the secret only in the server-to-server token request body.

Set multiple entries in the JSON array for multiple providers. `GET /api/auth/providers` publishes only enabled display names and login URLs; it never returns client secrets. A disabled provider cannot start login or complete a callback. Changing a provider ID creates a new identity namespace; keep IDs stable. Email matches never merge accounts.

Provider configuration is currently environment based; restart the API after changes. Keep the JSON value in a secret manager or protected environment file, never in `NEXT_PUBLIC_*` variables.
