# Workspace plugins (API v1)

Howllo Web has named extension points. A package is installed by the platform,
approved by the platform admin, and enabled by a workspace owner/admin. One
plugin per extension point can be active in a workspace. Disabling a package in
Platform settings turns off its workspace installations; a tenant must enable
it again after the platform admin reapproves it.

The first supported points are:

| Slot | What it can change | DOM hook |
| --- | --- | --- |
| `workspace.typography` | Public board fonts and text spacing | `[data-howllo-plugin-surface]` |
| `board.directory.layout` | Public board directory layout | `[data-howllo-slot="board.directory"]` |
| `board.topics.layout` | Topic-list density for one board | `.experience__forum` |
| `board.topics.typography` | Topic and thread reading style for one board | `.experience__forum`, `.post-thread` |

The topic slots are configured under **Workspace → Boards → Board appearance
and plugins**. Each board chooses its own approved plugin for each slot. The
workspace-wide typography and directory slots stay in **Workspace → Plugins**.
The public board presentation endpoint returns only approved and enabled
stylesheets, and checks private-board access before returning data.

Packages in v1 are stylesheet assets under `howllo-web/public/plugins/`. They
are versioned in `plugin_catalog`. Add a migration for a new package with its
ID, version, name, description, slot and stylesheet path. The migration should
set `is_approved=FALSE`; a platform admin reviews and approves it in **Platform
settings → Plugins**. A tenant then enables it in **Workspace → Plugins**.
Tenant managers cannot provide stylesheet URLs or arbitrary JavaScript.

Code highlighting, custom renderers and third-party widgets need a reviewed
renderer and isolation model before they can run on visitor pages. The v1
plugin system intentionally supports vetted CSS packages only. Core posting,
replies, voting, categories, tags, search, pinning and sorting do not depend
on plugins.

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

Howllo Web loads only approved stylesheet paths under `/plugins/` for the
current workspace. A new slot or API version requires an explicit core change
and documentation before packages can rely on it. JavaScript interactions are
reserved for a future isolated extension mechanism.
