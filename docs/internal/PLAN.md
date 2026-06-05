# Howllo Project Plan

## 1. What Howllo Is

Howllo is a lightweight product feedback and roadmap platform.

It helps product teams collect signals from users, organize them, discuss them, and turn them into product decisions.

It is not meant to be a traditional forum first.
It is not meant to be a generic community platform first.

The core idea is:

- users submit signals
- teams review signals
- teams update status
- signals become roadmap items
- users can see progress clearly

Howllo should feel closer to:

- product feedback board
- roadmap tracker
- discussion layer around product decisions

than to:

- old-style webboard
- Reddit clone
- generic forum engine


## 2. Product Positioning

Primary positioning:

> Hear what your users are saying.

Secondary positioning:

> Turn user feedback into product direction.

Howllo should be presented as:

- feedback collection
- roadmap visibility
- lightweight discussion
- team decision surface

It should not be presented as:

- a full social network
- a giant forum suite
- a general-purpose CMS


## 3. Product Scope

### Core problem

Teams often have scattered feedback across chat, email, tickets, and random conversations.
Users do not know where to ask for features.
Teams do not have a clean way to show what is planned, in progress, or done.

Howllo solves that by giving one clear place for:

- feature requests
- bug reports
- product discussion
- roadmap updates

### Core user types

#### End users

People who:

- request features
- report issues
- vote on ideas
- comment on posts
- follow product progress

#### Product/admin users

People who:

- review submissions
- respond officially
- merge duplicates
- change statuses
- move signals into roadmap states

#### Tenant owners

People who manage one product space or organization.


## 4. Non-Goals for V1

The following should not be first-priority in V1:

- full forum engine
- plugin marketplace
- complex customization system
- per-tenant billing system
- advanced moderation workflows
- full enterprise permissions matrix
- rich AI automation everywhere
- custom markdown editor with many advanced features
- file-heavy collaboration features
- knowledge base / wiki replacement

If a feature does not directly improve the feedback-to-roadmap loop, it is not a V1 priority.


## 5. Product Model

Howllo should be a standalone product.

Recommended deployment model:

- standalone app
- separate repo from Rooiam
- separate service from other products
- tightly integrated with Rooiam for authentication

Recommended relationship:

- Rooiam = auth / identity / SSO layer
- Howllo = feedback / roadmap / discussion layer

This keeps security-sensitive auth logic separated from product discussion logic.


## 6. Open Source Strategy

Recommended strategy:

- open core
- hosted SaaS on top

### Open source part

Open source should include the core feedback board:

- tenants
- boards
- posts
- comments
- votes
- status changes
- basic admin controls
- public UI
- self-hostable deployment

### Closed / hosted value

Keep these as hosted or commercial features later:

- advanced analytics
- multi-tenant onboarding UX
- managed email notifications
- Slack / Discord / webhook integrations
- AI-powered duplicate detection at scale
- semantic clustering
- advanced moderation tools
- enterprise SSO management
- audit logs
- team permissions beyond the basics
- custom domain automation
- private boards
- premium branding removal / advanced theming

### Why open core

Open core is a good fit because:

- it builds trust
- it gives developer adoption
- it becomes marketing
- it matches the founder’s ecosystem
- it gives Rooiam a real dogfooding story

### Guiding rule

Open source should make the product useful.
Hosted/commercial features should make it easier, smarter, and better to operate.


## 7. Tenant Strategy

Recommended direction:

- multi-tenant in architecture from day one
- simple operationally in V1

That means:

- every important record should have `tenant_id`
- internal design should assume multiple tenant spaces
- public launch can still start with only a few owned tenants

Examples of first tenants:

- Rooiam
- AraiHub
- Seavanna

### Why multi-tenant now

Because this product is likely to be reused across multiple products.
Rebuilding or migrating from single-tenant later will be more painful than starting with tenant boundaries now.

### What to avoid in V1

Do not add too much tenant complexity early:

- no tenant billing engine yet
- no deep theme builder yet
- no per-tenant auth provider matrix yet
- no complex role matrix yet

Architecturally multi-tenant.
Business-complexity-wise still simple.


## 8. Pricing Direction

### Free tier

Free should be generous enough to drive adoption.

Suggested free tier:

- public boards
- feature requests
- comments
- voting
- roadmap statuses
- basic moderation
- powered by Howllo branding

### Paid tier later

Suggested paid features:

- remove branding
- private boards
- custom domain
- basic analytics
- email notifications
- advanced moderation
- integrations
- multiple admins / roles
- SSO and enterprise controls

### Early-stage rule

Do not over-focus on pricing before product usage is proven.
Get real usage and feedback first.


## 9. V1 Core Feature Set

V1 should be intentionally small.

### Must-have

- tenant
- board
- post
- comment
- vote
- tags
- status
- official response marker
- basic admin moderation
- public list page
- detail page
- create post form
- sign-in via Rooiam

### Status values

Recommended initial statuses:

- under-review
- planned
- in-progress
- done
- declined

### Board types

Recommended initial board types:

- feature-requests
- bug-reports
- discussions
- announcements

### Sorting

Recommended list sorts:

- top
- newest
- active

### Minimal moderation controls

- hide post
- lock post
- mark duplicate
- merge duplicate later
- mark official comment


## 10. V1 User Experience Direction

Howllo should feel:

- calm
- clear
- lightweight
- structured
- product-focused

It should not feel like a noisy social feed.

### UI mental model

Users are not creating “forum topics”.
Users are sending “signals”.

That framing is useful for product identity.

Suggested wording direction:

- create signal
- top signals
- active signals
- roadmap
- official update

This can evolve later, but the signal language is strong and differentiating.


## 11. Technical Direction

### Recommended stack

- Rust
- Actix Web
- PostgreSQL
- SQLx
- Yew
- Docker for deployment

### Services

Suggested services:

- `howllo-server`
- `howllo-web`
- PostgreSQL
- optional background worker later

### Authentication

Use Rooiam for auth from the start.

Recommended approach:

- Howllo is an OIDC client of Rooiam
- user identity comes from Rooiam
- Howllo manages tenant membership and board-level authorization separately

### Why this matters

This gives:

- real Rooiam dogfooding
- consistent login
- less auth logic duplicated inside Howllo


## 12. Suggested Data Model

High-level initial entities:

### tenants

Represents one product space.

### users

Represents signed-in identities inside Howllo.
May reference Rooiam subject / identity mapping.

### memberships

User membership in tenant.

### boards

Tenant-scoped boards such as feature requests or bug reports.

### posts

Core content item.

### comments

Replies to posts.

### post_votes

Vote relation.

### tags

Tenant-scoped tags.

### post_tags

Many-to-many relation between posts and tags.

### post_status_history

Audit trail for status changes.

### notifications

Optional later.

### official_responses

Optional table or inline flagging approach.

### duplicate_links

Optional later for duplicate merging.


## 13. Repository Direction

Suggested repo layout:

- `howllo-server`
- `howllo-web`
- `howllo-docs` or `/docs` inside main repo

Suggested docs in project folder:

- `PROJECT_PLAN.md`
- `ROADMAP.md`
- `ARCHITECTURE.md`
- `AI_DIRECTION.md`
- `OPEN_SOURCE_STRATEGY.md`
- `PRICING_NOTES.md`

At minimum, this document can later be split into those files.


## 14. Roadmap

## Phase 0: Foundation

Goal:
Define the product clearly and avoid feature drift.

Deliverables:

- name locked
- domain locked
- initial positioning locked
- repo structure decided
- auth integration direction decided
- initial schema designed
- initial route list designed

Exit condition:

- clear MVP scope exists
- no open strategic confusion remains


## Phase 1: MVP

Goal:
Ship the smallest useful version.

### Deliverables

- tenant model
- boards
- post creation
- comments
- voting
- tags
- statuses
- public browsing
- admin status updates
- Rooiam login
- basic moderation

### Success criteria

- founder can use it for Rooiam feedback
- users can submit and vote without confusion
- roadmap states are visible
- admin can manage signals without manual database work


## Phase 2: Product Usability

Goal:
Make it feel polished and product-like.

### Deliverables

- better list filters
- post search
- official response display
- duplicate suggestion UX
- board navigation improvements
- notification preferences
- better admin workflow
- announcements board
- roadmap page

### Success criteria

- feedback flow feels smooth
- users can discover existing requests before posting duplicates
- roadmap becomes one of the main views


## Phase 3: Smarter System

Goal:
Improve signal quality and reduce noise.

### Deliverables

- duplicate clustering
- semantic related-post suggestions
- AI-assisted summaries
- spam heuristics
- moderation assist
- board health metrics
- trend detection

### Success criteria

- lower duplicate rate
- faster admin review
- clearer signal prioritization


## Phase 4: Hosted Product

Goal:
Turn Howllo into a reusable SaaS.

### Deliverables

- hosted onboarding
- tenant creation UX
- custom domains
- billing
- paid plans
- advanced permissions
- private boards
- integrations

### Success criteria

- external teams can onboard
- free-to-paid conversion path exists
- hosting and tenant management are manageable


## 15. AI / LLM Direction

This section is important.

The core rule:

**AI should improve the signal workflow, not define the product.**

Howllo should remain useful without AI.
AI should be an enhancement layer, not a dependency for basic correctness.

### Strong recommendation

Do not build V1 around LLMs.
Ship deterministic core features first.

### Why

If the core system depends on AI too early, you risk:

- unstable behavior
- inconsistent UX
- higher operating cost
- harder debugging
- unclear product value

Howllo should first prove:

- people will submit signals
- teams will review them
- statuses and roadmap views matter
- duplicate noise is a real problem worth solving

Only then should LLM features become important.


## 16. AI Principles

### Principle 1

Core data and workflow must be deterministic.

Examples:

- posts
- votes
- comments
- statuses
- moderation actions

must not depend on LLM output.

### Principle 2

LLMs should propose, not decide.

Examples:

- suggest duplicate candidates
- suggest tags
- suggest summaries
- suggest board classification

But a human or deterministic workflow should confirm important actions.

### Principle 3

AI must reduce admin time.

If an AI feature does not clearly reduce moderator or product-team workload, it should not be prioritized.

### Principle 4

AI should not make users feel manipulated.

Do not over-automate public interactions in a creepy or misleading way.


## 17. Recommended AI Use Cases

These are the best AI features for Howllo.

### 1. Duplicate suggestion

When a user creates a new signal, suggest:

- possible duplicates
- related existing requests
- similar bug reports

Implementation direction:

- start with embeddings + vector similarity
- later optionally use LLM reranking

This is one of the strongest uses because it directly reduces board noise.

### 2. Thread summarization

Generate concise summaries of long discussions.

Useful for:

- admins
- roadmap review
- users who join late

Implementation direction:

- summarize comments into:
  - problem summary
  - requested outcome
  - key objections
  - current admin response

### 3. Tag suggestion

Suggest likely tags during or after post creation.

This reduces manual categorization cost.

### 4. Post classification

Suggest whether a signal is:

- feature request
- bug report
- question
- discussion

This is a good assistive feature.

### 5. Similar-signal clustering

Cluster related requests over time so admins can see themes.

This is useful once boards grow.

### 6. Sentiment / urgency assist

Not raw sentiment for marketing fluff.
Instead, extract useful product signal:

- recurring pain points
- frequency of requested feature
- severity signals in bug reports

### 7. Search enhancement

Use semantic search to find related posts and discussions.

This is likely more useful than flashy chat features.

### 8. Moderation assist

Detect likely spam, abuse, or low-quality duplicate content.

This should remain assistive, not fully autonomous at first.


## 18. AI Features to Avoid Early

Avoid these early because they are flashy but weakly valuable:

- AI chatbot as primary navigation
- AI auto-reply pretending to be product team
- AI-generated roadmap without human review
- AI deciding post priority automatically
- AI rewriting user posts too aggressively
- heavy “AI everywhere” branding

These features risk making the product feel fake or unreliable.


## 19. Recommended AI Technical Approach

### Stage 1

No LLM dependency in core flow.

Use:

- PostgreSQL full-text search
- deterministic filtering
- standard tagging and moderation workflows

### Stage 2

Add retrieval and embeddings.

Use:

- embeddings for similarity search
- nearest-neighbor lookup for duplicate suggestion
- semantic related-post matching

### Stage 3

Add LLM summarization and classification.

Use LLMs for:

- summary generation
- tag suggestion
- class suggestion
- admin assistance

### Stage 4

Add richer intelligence only after real usage data exists.

Examples:

- trending themes
- cross-board insights
- release-note drafting from resolved signals

### Model strategy

Keep the architecture model-agnostic.

Suggested abstraction:

- `EmbeddingProvider`
- `CompletionProvider`
- `ModerationProvider`

This lets you swap vendors or support local models later.


## 20. AI Cost and Reliability Rules

### Cost rule

AI must justify its own cost.

If a feature creates cost without clearly improving conversion, retention, or admin efficiency, it should stay optional.

### Reliability rule

LLM output should be treated as soft output.

That means:

- never silently overwrite core records
- always preserve original user content
- avoid hidden transformations

### Privacy rule

Make AI optional or configurable for self-hosted/open-source users if possible.


## 21. Suggested Launch Order for AI

### AI Milestone A

Related-post suggestion during post creation.

### AI Milestone B

Admin summary generation for long threads.

### AI Milestone C

Duplicate clustering.

### AI Milestone D

Trend and insight summaries across boards.

This order keeps AI aligned with real value.


## 22. Free vs Paid Direction for AI

Good split:

### Free

- core product without AI
- maybe light related-post suggestions if affordable

### Paid

- advanced summaries
- duplicate clustering
- semantic search
- trend reports
- admin intelligence tools

This makes AI part of the monetization story without making the core product unusable.


## 23. Success Metrics

Initial useful metrics:

- number of signals created
- vote rate per signal
- comment rate per signal
- percent of signals receiving admin response
- percent of signals moved into roadmap status
- duplicate rate
- active users per tenant
- number of returning voters
- time to first official response

AI-specific metrics later:

- duplicate reduction rate
- moderator time saved
- summary usefulness rating
- related-post click-through rate


## 24. Recommended Immediate Next Steps

### Step 1

Create repo and docs folder.

### Step 2

Lock MVP scope.

### Step 3

Design schema for:

- tenants
- boards
- posts
- comments
- votes
- tags
- memberships
- status history

### Step 4

Implement auth via Rooiam.

### Step 5

Ship the simplest working board for Rooiam itself.

### Step 6

Use it internally and publicly.

### Step 7

Only after real usage appears, start AI milestone A.


## 25. Final Strategic Summary

Howllo should be:

- standalone
- Rooiam-auth integrated
- multi-tenant by architecture
- simple in business complexity at first
- open-core
- feedback-and-roadmap focused
- AI-assisted later, not AI-dependent first

The most important rule is:

**Do not build a giant platform before proving the basic feedback loop.**

Ship the smallest version that lets users ask, vote, discuss, and see progress.

Then let real usage tell you what deserves automation.