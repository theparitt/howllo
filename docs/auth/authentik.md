# Authentik

Create an OAuth2/OIDC provider and application in Authentik. Register `https://YOUR_API/api/auth/callback/authentik` as the redirect URI. Put its exact issuer URL, client ID and optional client secret in an `authentik` entry in `HOWLLO_OIDC_PROVIDERS`. Configure RS256 ID token signing, authorization code flow and S256 PKCE. See [OIDC setup](oidc.md). Authentik uses the generic OIDC adapter.
