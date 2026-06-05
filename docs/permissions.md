# Permissions

Rooiam authenticates; Howllo authorizes. Role claims from the frontend are
never trusted — every action is checked in `howllo-server`.

## Roles (tenant-scoped)

| Role      | Can do                                                        |
| --------- | ------------------------------------------------------------ |
| owner     | everything, including tenant settings and member management  |
| admin     | manage boards, tags, members; moderate; change status        |
| moderator | moderate posts/comments; change status; official responses   |
| member    | submit feedback, vote, comment                               |

Roles live in `memberships (tenant_id, user_id, role)`, unique per pair.

## Authorization checks

Every request validates, in order:

1. tenant membership (`memberships` row exists)
2. tenant role sufficient for the action
3. board visibility (private boards require access)
4. action-level permission (e.g. only admin/moderator may set status)

## Board visibility

Boards have `is_private`. Private boards require explicit access; public boards
are readable by anyone who can see the tenant.
