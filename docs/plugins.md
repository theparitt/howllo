# First-party plugins

Howllo Web has named extension points. **Only the Howllo maintainer writes and
ships plugins with the application.** There are no third-party submissions,
tenant uploads, external plugin URLs or plugin marketplace. A package is added
in source code, approved by the platform admin, and enabled by a workspace
owner/admin. One package per extension point can be active in each workspace or
board. Disabling a package in Platform settings turns off its installations; a tenant must enable
it again after the platform admin reapproves it.

The supported points are:

| Slot | What it can change | Surface |
| --- | --- | --- |
| `workspace.typography` | Public board fonts and text spacing | `[data-howllo-plugin-surface]` |
| `board.directory.layout` | Public board directory layout | `[data-howllo-slot="board.directory"]` |
| `board.topics.layout` | Topic-list density for one board | `.experience__forum` |
| `board.topics.typography` | Topic and thread reading style for one board | `.experience__forum`, `.post-thread` |
| `board.post.markdown` | Bold, italic, lists, links and a preview while writing posts | Post editor and post body |
| `board.attachments.pdf` | PDF upload and an on-demand preview | Post attachments |
| `board.attachments.model` | Self-contained GLB upload and interactive 3D preview | Post attachments |

The board slots are configured under **Workspace → Boards → open a board →
Extra content and plugins**. Each board chooses its own approved plugin for each slot. The
workspace-wide typography and directory slots stay in **Workspace → Plugins**.
The public board presentation endpoint returns only approved and enabled
packages, and checks private-board access before returning data.

Each package has a CSS asset under `howllo-web/public/plugins/`, a compiled
manifest in `howllo-server/src/plugins/registry.rs`, and a catalog migration.
Component packages also have a fixed client implementation in
`howllo-web/components/board-plugin-content.tsx`, activated only through the
matching ID, version, slot, stylesheet, runtime kind and capabilities in
`howllo-web/lib/board-plugin-runtime.ts`. API data cannot name a new script or
run tenant-supplied code. The first-party manifest is the trust boundary;
database approval alone cannot install code.

New catalog entries start with `is_approved=FALSE`. A platform admin approves
one in **Platform settings → Plugins**, then a tenant enables it under
**Workspace → Boards → open a board → Extra content and plugins**. A platform admin can
disable the package globally. A tenant can turn it off for one board. New
Markdown text remains stored as plain text; turning off its plugin will show
the Markdown syntax until the plugin is enabled again.

The board presentation endpoint now returns `api_version: 2` and each enabled
package's `id`, `version`, `slot`, `runtime_kind`, `capabilities`, and
`stylesheet_path`. It is the public, read-only capability negotiation API.
For example, `GET /api/boards/ideas/presentation?tenant_slug=sample` can return:

```json
{
  "api_version": 2,
  "announcement": "",
  "sidebar_text": "",
  "footer_text": "",
  "plugins": [{
    "id": "simple-markdown",
    "version": "1.0.0",
    "slot": "board.post.markdown",
    "runtime_kind": "component",
    "capabilities": ["post.editor", "post.body"],
    "stylesheet_path": "/plugins/simple-markdown.css"
  }]
}
```

The context endpoint below remains API v1 for public workspace metadata and
style packages; it does not expose privileged settings or execute code.

The PDF and 3D plugins permit uploads only on public boards, up to 8 MB per
file and 10 attachments per post. The server verifies file signatures, board
enablement, platform approval and storage quota. GLB files must be version 2
and self-contained, with no external URI references. Preview loading is
on-demand; a download link remains if preview is unsupported or disabled.
Private boards still cannot accept attachments until private storage exists.

Tags (shown as `#tag`), categories, search, pinning, voting, image previews,
PNG/JPEG/GIF/WebP upload, and the plain text editor are core board features.
They remain usable when all plugins are off. Future first-party candidates:
syntax highlighting and math rendering for technical discussion, embedded
media with a strict allowlist, and richer document preview. Each requires its
own manifest capability, UI implementation, server-side validation where data
or upload behavior changes, and approval before tenants can enable it.

## Public data contract

`GET /api/plugins/v1/workspaces/{workspace_slug}/context` returns only data
already public for a published workspace. It requires no access token and
returns 404 for an unpublished workspace. Example:

```json
{
  "api_version": 1,
  "workspace": {
    "slug": "sample",
    "name": "Sample",
    "site_name": "Sample ideas",
    "accent_color": "#186789",
    "background_color": "#eaf5f8",
    "pages": { "boards": true, "feed": true, "roadmap": false }
  },
  "boards": [
    { "slug": "ideas", "name": "Ideas", "description": "New ideas", "board_type": "feedback", "header_image_url": null, "background_image_url": null }
  ],
  "plugins": [
    { "id": "board-grid", "version": "1.0.0", "slot": "board.directory.layout", "stylesheet_path": "/plugins/board-grid.css" }
  ]
}
```

The `boards` list excludes private, paused and unpublished boards. It is empty
when the workspace hides the board directory. The endpoint does not expose
member identities, credentials, IP addresses, moderation queues, drafts or
private settings. Plugin authors can use the existing public board and post
endpoints for more public data; all normal visibility rules still apply.

Howllo Web loads only approved first-party stylesheet paths under `/plugins/` for
the current workspace. A new slot or API version requires a coordinated server,
web, migration and documentation change.
