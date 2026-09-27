# Deploy Howllo

> [!IMPORTANT]
> The self-hosted release is still a preview. Review the [release gates](RELEASE_GATES.md) before using private boards or sensitive customer data.

## 1. Run the API and data services

Run `howllo-server` with PostgreSQL and either local upload storage or MinIO. Give the API a public HTTPS origin, and set the server's public URL and allowed frontend origins. The [self-host guide](https://docs.howllo.dev/self-host.html) walks through the board and sign-in setup.

The Docker [quickstart](../README.md#quickstart) binds ports to `127.0.0.1` for a local demo. It is not a public-server configuration.

## 2. Deploy the frontends to Cloudflare Workers

From the repository root:

```bash
npm ci
cp .env.production.example .env.production
# Edit .env.production with your public HTTPS API, App, and Web URLs.
npm run deploy:check
npm run deploy
```

Authenticate Wrangler with `npx wrangler login` or a Cloudflare API token first. `.env.production` is gitignored; shell environment values take precedence. `npm run deploy` publishes App, Web, Admin, Landing, and Docs as separate Workers. It does **not** deploy the Rust API, PostgreSQL, or storage.

`NEXT_PUBLIC_*` values are embedded into browser bundles at build time. Rebuild after changing domains. The API URL must be reachable from both browsers and Cloudflare Workers; a private `192.168.x.x` address or `localhost` is unsuitable for a public deployment.

## Example: Howllo's hosted routes

| Domain | Service |
| --- | --- |
| `api.howllo.dev` | Rust API through Cloudflare Tunnel |
| `app.howllo.dev` | Workspace staff app |
| `feedback.howllo.dev` | Public boards |
| `admin.howllo.dev` | Platform operator console |
| `howllo.dev`, `www.howllo.dev` | Landing site |
| `docs.howllo.dev` | Guides |

On the maintainer's host, the Cloudflare Tunnel routes `/howllo/*` to the private MinIO bucket and other `api.howllo.dev` paths to the Rust API. The `howllo-server.service` and `howllo-api-tunnel.service` units keep them running. Self-hosters can use their own domains and reverse proxy.

## Configure sign-in and limits

- Register the exact callback URL shown in Howllo's sign-in settings with each OAuth provider. See [OIDC setup](auth/oidc.md).
- Use **Admin → Platform settings → Limits** for platform defaults and workspace storage caps. Workspace owners use **App → Security** for allowed overrides.
- Country rules need Cloudflare's `CF-IPCountry` header. IP and country rules apply to posts, comments, and uploads.
- Configure SMTP in **Admin → Email** only when delivery is needed. See the [email guide](email.md).

See [architecture](architecture.md), [plugins](plugins.md), and the [release roadmap](roadmap.md) for deeper details.
