<h1 align="center">
  <img src="art/howllo-logo-wordmark-horizontal.png" width="380" alt="Howllo" />
</h1>

<p align="center">
  <strong>Open-source discussion boards for communities and product teams.</strong><br />
  Talk, ask questions, report bugs, suggest features, and share a roadmap.
</p>

<p align="center">
  <img src="art/howllo-cartoon.jpg" width="150" alt="Howllo wolf mascot" />
</p>

<p align="center">
  <a href="docs/roadmap.md"><img src="https://img.shields.io/badge/Release-preview-F2A65A?style=flat-square" alt="Preview release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-Apache--2.0-3178A8?style=flat-square" alt="Apache-2.0 license" /></a>
  <a href="https://github.com/theparitt/howllo/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/theparitt/howllo/ci.yml?branch=main&label=CI&style=flat-square" alt="CI status" /></a>
</p>

---

## Quickstart

**You need:** Docker, Git, curl, and OpenSSL on macOS or Linux.

```bash
git clone https://github.com/theparitt/howllo.git
cd howllo
./scripts/quickstart.sh
```

| Open | URL | What you get |
| --- | --- | --- |
| 🟠 **Board** | [localhost:7703](http://localhost:7703) | A live sample discussion board |
| 🔵 **Staff app** | [localhost:7702](http://localhost:7702) | Create and manage your workspaces |
| 🟢 **API** | [localhost:7700/api/health](http://localhost:7700/api/health) | Server health |

The script starts PostgreSQL, the API, and both web apps. It uses local sign-in and local file storage. Ports bind to `127.0.0.1` for a laptop demo.

### Make your own board

1. Open **Staff app** and create a local account.
2. Create a workspace, then add a board.
3. Publish the board and workspace to show them on **Board**.

New workspaces start empty and unpublished. The sample board is created only by the Quickstart.

### Stop or inspect

```bash
# See the API and web logs
docker compose --env-file .env.quickstart -f compose.quickstart.yml logs -f server web

# Stop; your data stays in Docker volumes
docker compose --env-file .env.quickstart -f compose.quickstart.yml down
```

---

## What is a Howllo board?

Each workspace can have several boards, with a layout suited to its purpose.

| Board type | For |
| --- | --- |
| **Discussions** | Topics, questions, and replies |
| **Feature requests** | Ideas, votes, and progress |
| **Bug reports** | Issues and reproduction details |
| **Announcements** | Updates from the team |

The **roadmap** is a separate workspace page that shows progress across posts. See [board types and controls](docs/board-types.md).

---

## Guides

| I want to… | Read |
| --- | --- |
| Run a public installation | [Deployment guide](docs/deployment.md) · [Visual self-host tutorial](https://docs.howllo.dev/self-host.html) |
| Change the code | [Development setup](docs/development.md) · [Architecture](docs/architecture.md) |
| Configure sign-in or email | [OIDC](docs/auth/oidc.md) · [Email](docs/email.md) |
| Understand plugins and upcoming work | [Plugins](docs/plugins.md) · [Roadmap](docs/roadmap.md) |

> [!IMPORTANT]
> Howllo is a preview release. Review the [release gates](docs/RELEASE_GATES.md) before using private boards or sensitive customer data.

Licensed under [Apache 2.0](LICENSE). Contributions are welcome.
