# Identity and workspace management providers

Howllo works without an external identity service. A fresh installation uses
Howllo accounts, invitations, workspace membership and sessions. OpenID Connect
can be enabled for sign-in without moving workspace permissions out of Howllo.

The optional identity-management integration is a **server-side integration**,
separate from board presentation plugins. A board plugin cannot grant access,
send invitations or manage sessions. Installing a board plugin must never change
the source of truth for users or permissions.

## Ownership of data

| Concern | Howllo | External identity provider |
| --- | --- | --- |
| Posts, votes, comments, boards, moderation, bans | Source of truth | No access |
| Workspace owner, admin and moderator permissions | Source of truth | May hold an organization membership, but its roles are not a substitute |
| Local accounts and recovery codes | Source of truth in the default installation | Not required |
| External account identity and login sessions | Stores a stable provider subject and a Howllo session | Source of truth for that provider's identity sessions |
| External invitations and organization membership | Stores a pending Howllo role grant and a linked provider ID | Sends and tracks the provider invitation and membership |

The key for an external user is `(provider_id, subject)`, where `subject` is a
stable, verified ID from the provider. Email is display/contact data; equal
email addresses do not link accounts or authorize a staff role. Howllo still
checks workspace membership, restrictions and board permissions on every
request. Revoking a provider session must also revoke the corresponding Howllo
session before the UI promises that the user is signed out.

## Provider boundary

The default `local` implementation keeps the existing invitation, account and
workspace session flows. An external implementation must expose these
capabilities to Howllo's server, with an explicit provider choice for each
workspace:

1. Validate a login and return a stable subject and the organization/workspace
   ID that admitted the user.
2. Send, list and revoke workspace invitations. Keep the requested Howllo
   staff role as a pending grant until membership is verified.
3. Page through organization members and look up one member by stable ID.
4. List and revoke a member's provider sessions for authorized workspace staff.
5. List and revoke only the caller's provider sessions through a user-scoped
   token held on the server, not an organization API key in the browser.
6. Report provider errors distinctly from empty member/session lists. A failed
   provider request must never silently fall back to local authorization.

The integration needs a one-to-one mapping from a Howllo workspace to the
provider organization/workspace. Howllo records a selected provider when a
workspace first appears in the bridge; removing that mapping later fails closed
instead of silently switching the workspace back to local invitations. A global
login client ID is not sufficient
for managing multiple Howllo workspaces. Store provider API keys as server
secrets scoped to their mapped workspace; never send them to App or Web.

An external invitation is not a Howllo permission grant. On an authenticated
callback, Howllo verifies the provider organization membership and subject,
then activates the pending Howllo role grant. Reconcile withdrawn, expired and
removed memberships and revoke active Howllo sessions when access disappears.
Keep an audit trail for role changes and reconciliation. An invitation API that
only lists pending invitations cannot prove that an invitation was declined;
the UI must not label it “declined” without a provider event or status API.

## RooIAM integration requirements

The current RooIAM JavaScript server SDK uses a workspace API key and supports
workspace invitations, member lists, member sessions and session revocation.
It does not provide a full history of accepted or declined invitations, and
its assignable `admin`/`member` roles do not represent Howllo's
`owner`/`admin`/`moderator`/`member` roles. Howllo therefore retains its product
roles and uses RooIAM organization membership as an identity admission signal.

The RooIAM browser SDK's self-session methods use RooIAM's own first-party
cookie. For Howllo on another origin, Howllo must hold the user's OIDC access
token server-side and call RooIAM's bearer-token self-session endpoints. The
workspace API key is only for authorized staff operations. The current
browser-held widget token flow must be replaced with a server-side session
before exposing self-service session management as a secure production feature.

RooIAM management is **not enabled merely by**
`HOWLLO_WORKSPACE_AUTH_PROVIDER=rooiam`: that setting selects the staff login
widget. It does not provision a RooIAM workspace, grant API permissions, sync
members or connect session management. Public-board RooIAM login is configured
separately per workspace. Do not describe a deployment as fully RooIAM-managed
until those management capabilities are connected and verified end to end.

The optional bridge is hidden on local installations and lives outside the
public repository. For a linked workspace, Howllo sends staff invitations
through the bridge and keeps the requested product role pending. The invitee
accepts in the identity provider first, then joins in Howllo App. Howllo checks
the provider invitation's accepted subject and active workspace membership
before granting the product role. The inviter sees provider invitation status
in App → Staff. Standalone workspaces retain Howllo's one-time-code flow.
Removing an account through App revokes its provider membership and Howllo
workspace access. Howllo revokes its own access first; if the provider call
fails, retry the removal in App after the provider recovers. Removals made
directly in the provider are not yet reconciled
with existing Howllo sessions. User self-session management remains unfinished;
these gaps must close before claiming full provider-managed access.

| Capability | Standalone Howllo | Hosted bridge today |
| --- | --- | --- |
| Staff and customer sign-in | Local or configured OIDC | Existing RooIAM widget login can be selected |
| Staff invitations and role grants | Howllo one-time code and Howllo roles | RooIAM invitation; Howllo grants the requested role after verifying the accepted subject and active membership |
| Sign-in account directory | Howllo staff/board member lists | Provider directory in App → Staff; workspace owners can remove a provider member and its Howllo workspace access |
| Staff view of member sessions | Howllo sessions | Optional provider session list; workspace owners can revoke them and linked Howllo workspace sessions |
| End-user self-session management | Howllo account logout | Pending server-held OIDC access-token flow |

This is a migration checkpoint, not a claim that RooIAM management is complete.

## Rollout and migration

1. Keep `local` the default, including for newly created workspaces. Existing
   local accounts, invitations and sessions continue to work.
2. Provision a distinct RooIAM workspace and scoped integration key for each
   Howllo workspace opting in. Verify the key belongs to the expected RooIAM
   workspace before enabling it. Store the mapping and credentials on the
   server only.
3. Reconcile identities using verified provider subjects. Review accounts
   with ambiguous email matches manually; never merge them automatically.
4. Enable RooIAM login, then invitation/member read operations, then session
   operations, and finally provider-backed writes. Test each step with owner,
   admin, moderator, member and banned-user accounts.
5. Test invite acceptance/withdrawal/expiry, role changes, removal, session
   revocation, provider outage, cross-workspace keys and stale cached data.
   The service must fail closed on permission and identity checks.

There is no reason for the open-source default install to fetch a RooIAM SDK,
create a RooIAM account or contact a RooIAM endpoint.
