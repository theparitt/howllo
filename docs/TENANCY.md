# Tenancy — Account → Workspaces

Howllo becomes a two-level tenancy: an **Account** (the customer) owns many
**Workspaces**; each workspace has its own boards, staff, settings, branding and
public board site. This is Stage 0 — it must land before invitations, because
invitations and staff are scoped to a workspace *within* an account.

## Terms (and how they map to today's code)

| Concept | Meaning | Today |
|---|---|---|
| **Account** (organization) | The customer / billing entity. Created on sign-up. Owns many workspaces. | **new** |
| **Workspace** | Boards + staff + settings + branding + public site. | this is what the DB calls **`tenants`** |
| **Board** | A feedback board inside a workspace. | `boards` (per workspace) |
| **Staff / member** | A person's role **in a workspace** (`owner/admin/moderator/member`). | `memberships` (per `tenant_id`) |

Decision: **keep the DB table name `tenants` = "workspace"** to avoid churn
across ~all queries; "workspace" is the user-facing word. Add `accounts` above it.

## Schema

```sql
CREATE TABLE accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    owner_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE tenants ADD COLUMN account_id UUID REFERENCES accounts(id) ON DELETE CASCADE;
-- backfill (below), then set NOT NULL.

-- Account-level roles (who can manage the account & all its workspaces).
-- MVP could use accounts.owner_user_id only; this table is the extensible form.
CREATE TABLE account_memberships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL DEFAULT 'owner',   -- owner | admin | billing
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (account_id, user_id)
);
```

Workspace slugs stay **globally unique** → public URLs `/{workspace}/…` are
unchanged; the account never appears in a public URL.

## Authorization / scoping

A user may act on workspace *W* if **either**:
- they hold an `account_memberships` row on *W*'s account (owner/admin) — implicit
  full rights on every workspace in the account; **or**
- they hold a `memberships` row on *W* directly (workspace staff).

Changes:
- `listTenants()` (admin `WorkspaceSwitcher`) → returns workspaces the caller can
  reach: `account_memberships`-owned ∪ `memberships`-staffed. (Today it returns
  *all* tenants only because the local-admin is global.)
- **local-admin stays global** — that's the Howllo *system* admin, out of scope.
- `require_permission` etc. gain an "account owner/admin ⇒ treat as workspace
  owner" shortcut so account owners don't need an explicit membership per workspace.

## Workspace creation & management

- Account owner/admin can **create** a workspace under their account
  (`POST /api/admin/workspaces {name, slug}`) → inserts `tenants(account_id=…)`
  + seeds default boards + gives the creator `owner` membership.
- Existing per-workspace management (boards/posts/members/settings) is unchanged;
  it now simply lives under an account.

## Migration / backfill (safe, no visible change)

1. Create `accounts` + `account_memberships` + `tenants.account_id` (nullable).
2. For each existing workspace, create a **1:1 account** (`account.name =
   tenant.name`, `slug = tenant.slug || '-org'`), owner = the workspace's current
   `owner` membership user; insert an `account_memberships(owner)` row; set
   `tenants.account_id`.
3. `ALTER TABLE tenants ALTER COLUMN account_id SET NOT NULL`.

Result: every current workspace keeps working exactly as-is, now owned by its own
account. Owners can then create additional workspaces under that account.

## What this does NOT change

- Boards, posts, votes, comments, follows, notifications — all still keyed by
  workspace (`tenant_id`). No data re-keying.
- The public board site and its routing.
- Staff roles remain **per workspace** (per the earlier decision — not per board).

## Revised roadmap

- **Stage 0 — Account layer (this doc):** schema + backfill, account-scoped
  `listTenants`, account-owner shortcut in authz, create-workspace API, admin
  account context. *(then everything below is unchanged)*
- **Stage 1 — Workspace invitations** (per workspace, under the account).
- **Stage 2 — Notifications engine.**
- **Stage 3 — Web notification center + toasts.**
- **Stage 4 — Activity feed home page.**

## Open decisions

- **Term:** "Account" vs "Organization" for the top level. (Default: **Account**.)
- **Account roles now or later:** ship `account_memberships` table but only use
  `owner` at first? (Default: **yes** — table now, single owner role first.)
- **Backfill grouping:** 1 account per existing workspace (default, safest) vs
  group a user's workspaces into one account.
