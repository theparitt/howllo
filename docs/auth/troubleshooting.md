# Authentication troubleshooting

- **No sign-in options:** Check `GET /api/auth/providers`, `HOWLLO_AUTH_LOCAL_ENABLED` and `HOWLLO_OIDC_PROVIDERS`. Restart the API after editing env.
- **OIDC login redirects to an error:** Check the provider's exact issuer, discovery document, registered API callback URL, client type and client secret. Howllo accepts RS256 ID tokens.
- **Invalid state:** Start login again in the same browser. The callback needs the short-lived SameSite cookie sent to the API host; browser and reverse proxy must preserve it.
- **Wrong audience or issuer:** Ensure the ID token is issued for Howllo's client ID and exactly matches the configured issuer. Do not loosen validation.
- **Unknown or disabled provider:** Verify the provider ID and `enabled` setting. Disabled providers are hidden and blocked server-side.
- **Old RooIAM widget missing:** Check the optional widget settings and the RooIAM redirect registered at `/auth/callback`. Generic OIDC uses the API callback instead.
- **User appears twice:** Identities are intentionally separate by provider and subject. Matching emails are never merged automatically; explicit account linking is not yet available.
- **Local login rejected:** Usernames are case-normalized; passwords need at least 12 characters on registration. Repeated attempts are rate limited.

Server auth logs include provider ID and event category, never tokens or client secrets.
