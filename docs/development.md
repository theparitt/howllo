# Develop Howllo from source

For a ready-to-run board, use the [Docker quickstart](../README.md#quickstart). This page is for editing the code locally.

## What runs where

| Service | Port | Purpose |
| --- | ---: | --- |
| `howllo-server` | 7700 | Rust API and business rules |
| `howllo-admin` | 7701 | Platform operator console |
| `howllo-app` | 7702 | Workspace and board management |
| `howllo-web` | 7703 | Public boards |
| `howllo-landing` | 5114 | Product site |
| `howllo-docs` | 5116 | Guides |

## Start the API

Install Docker and a Rust toolchain. From the repository root:

```bash
docker compose up -d
cp howllo-server/.env.example howllo-server/.env
cd howllo-server
cargo run
```

The root Compose file starts PostgreSQL on `localhost:5433` and MinIO on `localhost:9000`. The server applies its migrations at startup. The example environment uses PostgreSQL and local file storage; configure `HOWLLO_MINIO_*` only when you want MinIO uploads.

## Start the frontends

Install Node.js 22. In a second terminal, from the repository root:

```bash
npm ci
npm run dev:app
```

In a third terminal:

```bash
npm run dev:web
```

Open the staff app at <http://localhost:7702> and public web at <http://localhost:7703>. Optional development servers:

```bash
npm run dev:admin    # localhost:7701
npm run dev:landing  # localhost:5114
npm run dev:docs     # localhost:5116
```

## Change the API or storage address

- Set `HOWLLO_DATABASE_URL` in `howllo-server/.env` for another PostgreSQL server. Keep `DATABASE_URL` in sync if you use the SQLx CLI.
- Copy `howllo-app/.env.example` and `howllo-web/.env.example` to `.env.local` in their respective directories, then set `NEXT_PUBLIC_API_BASE_URL` to a URL the browser can reach.
- Add both frontend origins to `HOWLLO_ALLOWED_ORIGINS` in the server environment.
- Set `HOWLLO_STORAGE_PUBLIC_BASE_URL` to the browser-accessible upload URL. Storage settings saved in PostgreSQL override environment defaults.

## Sign-in and email

Fresh installations use local passwords and a one-time recovery code. Staff can share single-use invitation codes when email is disabled. For optional providers and delivery, see [OIDC](auth/oidc.md) and [email setup](email.md).

Each workspace can contain multiple boards. The public URL is `/{workspace}/boards/{board}`; internal records use UUIDs. See [board types](board-types.md) and [product surfaces](product-surfaces.md).
