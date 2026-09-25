# Howllo product surfaces

Howllo uses **one set of workspaces, boards, posts, and users** in the server and PostgreSQL. Management and customer views are separate interfaces over those records; a board is not copied or synchronized between them.

| Surface | Audience | Current local URL | Responsibility |
| --- | --- | --- | --- |
| Howllo App | Workspace owners and admins | `http://localhost:7702/app/{workspace}` | RooIAM sign-in; create and configure boards, team, workspace sign-in, and share public links |
| Howllo Web | End users | `http://localhost:7703/{workspace}` | Browse public boards; Howllo local account sign-in with recovery codes to post, vote, and comment |
| Howllo Admin | Platform operators | `http://localhost:7701/admin` | Platform settings and cross-workspace operations using the local operator account |

The first two surfaces are separate Next.js deployments. Howllo App uses the existing RooIAM callback on port 7702. Howllo Web uses local credentials and does not load the RooIAM widget. `howllo-admin` is not the tenant app: its local operator password and platform-wide access must not be given to tenants.

An account (organization) owns multiple workspaces. A workspace is one project with its own boards, feed, roadmap, staff, and public users. Staff are invited to a specific workspace as admin or moderator and accept in Howllo App using RooIAM. Public users join a specific workspace from Howllo Web using local accounts. App separates staff from public users; owners and admins may suspend a public user for 1–365 days or ban them until manually lifted. The restriction covers participation across all boards in that workspace, including existing sessions. Public boards remain readable without signing in.

Howllo App keeps a blue staff badge and cool background so it remains recognizable across workspaces. In App → Public branding, each workspace owner/admin can set the end-user site's name, upload a raster logo, choose accent/background colors, and control roadmap and attribution visibility. Board editors can also upload an icon and choose a board background color. Public Web reads those settings from the workspace branding and board records; branding changes do not alter Howllo App's staff identity.

Both frontends call the same Howllo API. Board identity is `(workspace slug, board slug)`; the API resolves and authorizes both together. Each public URL contains the workspace slug, so two workspaces may each have a board named `bug-reports` without seeing each other's posts. Keep authorization on the Howllo server: owner/admin for management APIs, public reads for public boards, and authenticated workspace sessions for posting. Do not transfer bearer tokens through public URLs.

## User flow

1. A signed-in owner opens a workspace from the account home and lands in Howllo App.
2. They create a public board in Boards, configure it, and copy its public link.
3. A customer opens the Howllo Web workspace directory or the direct board link. They can read a public board without an account.
4. The customer signs in with a Howllo local account to create posts, vote, or comment. A one-time recovery code is shown at registration. Changes made by the owner appear on the same board immediately because both views use the same API and records.

Private boards do not appear in the public workspace directory and require the server's private-board authorization check for direct access.
