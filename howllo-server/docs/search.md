# Search

Full-text search powered by PostgreSQL `tsvector` and GIN index.

## Endpoint

`GET /api/search/posts?tenant_slug=acme&q=dark+mode`

## Parameters

| Param | Description |
|-------|-------------|
| `q` | Search query (required) |
| `tenant_slug` | Tenant identifier (required) |
| `tag` | Filter by tag slug |
| `status` | Filter by status |
| `board_slug` | Filter by board |
| `sort` | Ranking mode |

## Ranking Modes

| Mode | Description |
|------|-------------|
| `relevance` | PostgreSQL `ts_rank` ordering (default) |
| `newest` | Most recent first |
| `top` | Most voted first |
| `active` | Recently updated |
| `most_voted` | By vote count descending |
| `most_commented` | By comment count descending |

## Visibility Rules

Search respects all visibility rules:
- Hidden posts excluded
- Deleted posts excluded
- Private board posts excluded for anonymous users
- Private board posts included for authorized members

## Duplicate Suggestion

`GET /api/boards/{board_slug}/suggest-duplicates?tenant_slug=acme&q=title+text`

Returns up to 5 similar posts using the search engine. Automatically excludes hidden, deleted, and private posts that the user cannot see.
