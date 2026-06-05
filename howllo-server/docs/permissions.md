# Permissions

## Roles

| Role | Description |
|------|-------------|
| **owner** | Full access to everything |
| **admin** | Manage boards, tags, webhooks, API tokens, members |
| **moderator** | Hide posts/comments, mark duplicates, lock posts |
| **member** | Create posts, vote, comment, follow |

## Permission Matrix

| Action | owner | admin | moderator | member | anon |
|--------|-------|-------|-----------|--------|------|
| View public board | ✓ | ✓ | ✓ | ✓ | ✓ |
| View private board | ✓ | ✓ | ✓ | ✓ | ✗ |
| Create post | ✓ | ✓ | ✓ | ✓ | ✗ |
| Edit own post | ✓ | ✓ | ✓ | ✓ | ✗ |
| Vote | ✓ | ✓ | ✓ | ✓ | ✗ |
| Comment | ✓ | ✓ | ✓ | ✓ | ✗ |
| Follow | ✓ | ✓ | ✓ | ✓ | ✗ |
| Hide post | ✓ | ✓ | ✓ | ✗ | ✗ |
| Hide comment | ✓ | ✓ | ✓ | ✗ | ✗ |
| Change status | ✓ | ✓ | ✗ | ✗ | ✗ |
| Mark duplicate | ✓ | ✓ | ✓ | ✗ | ✗ |
| Lock post | ✓ | ✓ | ✓ | ✗ | ✗ |
| Soft delete | ✓ | ✓ | ✗ | ✗ | ✗ |
| Create board | ✓ | ✓ | ✗ | ✗ | ✗ |
| Create webhook | ✓ | ✓ | ✗ | ✗ | ✗ |
| Create API token | ✓ | ✓ | ✗ | ✗ | ✗ |
| Manage members | ✓ | ✓ | ✗ | ✗ | ✗ |
| View audit log | ✓ | ✓ | ✗ | ✗ | ✗ |
| Export data | ✓ | ✓ | ✗ | ✗ | ✗ |

## Visibility Rules

- Hidden posts are invisible to non-admin/moderator users
- Soft-deleted posts return 404
- Private board content is invisible to non-members
- Cross-tenant access returns 404 (not 403)

## API Token Scopes

| Scope | Access |
|-------|--------|
| `posts:read` | Read posts and boards |
| `posts:write` | Reserved for future |
| `comments:read` | Read comments |
| `comments:write` | Reserved for future |
| `roadmap:read` | Read roadmap |
| `webhooks:write` | Manage webhooks |
| `admin:read` | Reserved for future |
