# Role-based UI flow audit — 27 September 2026

This audit used separate Chromium browser contexts against an isolated local API, PostgreSQL database, App, and Web (`127.0.0.1:17700`, `:17702`, `:17703`). It did not change the live `770x` stack or production data. The account identities, invitation codes, and posts used for the audit live only in the isolated database.

## Journeys exercised through the UI

| Role | Start-to-finish journey | Result |
| --- | --- | --- |
| Workspace owner | Register local account, save one-time recovery code, create an empty workspace, create/enable a Feature Ideas board, publish the workspace | Passed. A new workspace has no boards and begins unpublished. |
| Workspace owner | Create and publish Bug Tracker, Community Discussions, and Product Updates boards; inspect distinct public layouts | Passed. Optional board image/color controls were collapsed to keep the main editor shorter. |
| Owner + staff | Invite moderator with a one-time code; staff registers and accepts; owner sees Accepted status and accepted account name; staff reaches moderator controls | Passed. Owner receives an invitation acceptance notification in the App header. |
| Owner + staff | Owner promotes moderator to admin; staff reloads and gains admin controls; owner removes staff; staff's direct workspace URL and root list deny access | Passed. The API now also protects owner roles and the final owner from direct role-change/removal requests. |
| Owner + staff | Owner creates a second invite; staff cancels then confirms decline; owner sees Rejected status and notification; declined code is reused | Passed. Cancel preserves the code; confirmed decline does not grant membership; replay fails. |
| Staff | Register, save recovery code, reset password, sign in with new password, try wrong password | Passed. The one-time recovery code rotates. |
| Public visitor + two customer accounts | Browse, search, filter, sort, sign up, preserve a draft across login, create feature idea, vote/unvote, follow/unfollow, comment | Passed. Vote state survives reload; the first customer receives the second customer's comment notification. |
| Customer | File structured bug report; start a discussion and reply; read a Product Updates announcement and comment | Passed. Customer cannot create an announcement. |
| Mobile customer | Browse board, create form, and post at 390 px width | Passed with no horizontal overflow in inspected pages. |
| Fresh installation | Open Web root while default workspace remains unpublished | Passed. Shows “No public boards yet” instead of a dead 404. |
| Customer + moderator + visitor | Enable manual post review, submit a post, approve/reject from Staff App, inspect guest visibility | Passed. Pending and rejected posts are absent and return 404 to guests; approved posts become visible. |
| Moderator | Search all posts in Staff App; change status with note, post official reply, pin/unpin, lock/unlock, hide/show | Passed. Reversible actions were restored; official reply was verified in post comments. Owner-only APIs returned 403. |
| Workspace owner | Save/reload Board site, Security defaults, external SSO toggle, plugins, and customer restriction; restore original values | Passed where noted in owner follow-up. Invalid hourly/daily combinations are now explained inline. |

## UI problems found and changed

- Removed a duplicate staff Sign in action on the App root and reduced invitation-code entry to a clear optional control.
- Added concise contextual help for invitation codes, role/access settings, board visibility, and the Hot/Top sort choices. Invitation text now explains that possession of an unverified code authorizes joining.
- Owner Team view refreshes invitation status and identifies the account that accepted. A persistent App header bell shows invitation acceptance or rejection outside the Team tab.
- Simplified board editor labels and collapsed optional images/colors. The board list no longer stretches to the editor's height.
- Removed a duplicate board category filter; kept status filtering when sort changes. Guest users no longer see the personal Feed link.
- Preserved post and comment drafts across sign-in, showed clear prompts for guest actions, and made Vote/Follow buttons show their actual state.
- Updated notification and Feed links to canonical workspace paths, refreshed bell/Feed on account changes, and hid the unused invitation filter for customers without invitations.
- New workspaces hide Feed and Roadmap until the owner enables them. Existing workspaces retain their settings.
- Staff moderation now has a tenant-scoped post browser in App because staff sessions do not cross from App to Web. It includes review queue, all-post search, and direct post actions; the server checks moderation permission on the new listing endpoint.
- Password-only customer sign-in settings now load even when the platform has not configured a public API URL for social login. The page explains why social providers cannot be added yet.

## Verification and boundaries

- Server integration suite: 152 passed, 0 failed, using a second isolated test database. This includes invitation replay/rejection, role restrictions, board visibility, post and comment controls, rate limits, email behavior, and tenant isolation.
- Both App and Web passed TypeScript checks and optimized Next.js production builds. `git diff --check` passed.
- Browser tests used local-account auth. The RooIAM callback and external OIDC login, actual outbound SMTP delivery, and production deployment were not exercised in this audit.
- The owner saw invitation changes on cold load and via manual refresh. The 20-second Team-tab polling interval was not independently timed while two actors kept their pages open.
- The manual review setting applies to posts. Comments on approved posts publish immediately under separate spam limits, matching the setting's label.
- The new moderation browser was tested with a small result set; pagination beyond the first page was covered by the server API test, not a browser with over 20 posts. The rewritten Reject confirmation is present in the UI; rejection itself was browser-tested before that UI rewrite.
- Official replies currently use two existing API calls: create a comment, then mark it official. If the second call fails, the first may remain a regular comment. This needs an atomic server operation before treating that failure mode as solved.
