# Overview

Howllo Server is a feedback management backend for SaaS products. It provides multi-tenant boards where users can submit, vote on, and comment on feedback. Administrators can moderate submissions, track status on a public roadmap, and integrate via webhooks and API tokens.

## Core Concepts

### Tenants
Each tenant is an isolated workspace with its own boards, posts, members, and settings. No data leaks between tenants.

### Boards
Boards are categories for feedback. Each board belongs to a tenant and can be public or private. Board types include `feature-requests`, `bug-reports`, `changelog`, and `internal`.

### Posts
Posts are individual feedback items. Each post has a status: `under_review`, `planned`, `in_progress`, `done`, or `declined`. Posts can be voted on, commented on, followed, tagged, and marked as duplicates.

### Status Flow
```
under_review → planned → in_progress → done
     ↓           ↓           ↑
  declined    declined    planned
     ↓
  under_review
```

### Roadmap
The public roadmap shows only `planned`, `in_progress`, and `done` posts. Hidden, deleted, declined, and duplicate posts are excluded. Private board posts are only visible to members.

## Key Design Principles

- **Backend owns all visibility rules** — Frontend never decides what to hide
- **AI is advisory only** — AI suggests, humans approve
- **Every mutation is audited** — Admin/moderator actions are traceable
- **API tokens are read-only** — Write operations require JWT authentication
