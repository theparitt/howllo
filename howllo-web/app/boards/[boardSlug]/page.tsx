import Link from "next/link";
import { getBoardCategories, getBoardDetail, getBoardPosts, getBoardPresentation, getTags } from "@/lib/api";
import {
  WorkspaceContextError,
  buildTenantPath,
  resolveTenantContext,
} from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";
import { ApiUnavailable } from "@/components/api-unavailable";
import { RealtimeRefresh } from "@/components/realtime-refresh";
import { StatusPill } from "@/components/status-pill";
import { WorkspaceState } from "@/components/workspace-state";
import { countLabel } from "@/lib/format";
import { ApiError } from "@/lib/api";
import { boardKind } from "@/lib/board-experience";
import { notFound } from "next/navigation";

type BoardPageProps = {
  params: Promise<{
    boardSlug: string;
  }>;
  searchParams: Promise<{
    tenant?: string;
    sort?: string;
    status?: string;
    category?: string;
    tag?: string;
    q?: string;
    page?: string;
  }>;
};

export default async function BoardPage({ params, searchParams }: BoardPageProps) {
  const { boardSlug } = await params;
  const query = await searchParams;
  let tenant: string;
  let defaultTenantSlug: string;

  try {
    ({ tenantSlug: tenant, defaultTenantSlug } = await resolveTenantContext(query.tenant));
  } catch (error) {
    if (error instanceof WorkspaceContextError) {
      return <WorkspaceState kind={error.kind} workspaceSlug={error.workspaceSlug} />;
    }
    throw error;
  }

  const sort = query.sort ?? "newest";
  const status = query.status;
  const page = Math.max(1, Number(query.page ?? "1") || 1);
  const token = await getServerBearerToken(tenant);

  try {
    const [board, postsPage, categories, tags, presentation] = await Promise.all([
      getBoardDetail(tenant, boardSlug, token),
      getBoardPosts({
        tenantSlug: tenant,
        boardSlug,
        sort,
        status,
        category: query.category,
        tag: query.tag,
        q: query.q,
        page,
        perPage: 20,
        token,
      }),
      getBoardCategories(tenant, boardSlug, token),
      getTags(tenant),
      getBoardPresentation(tenant, boardSlug, token),
    ]);
    const posts = postsPage.items;
    const boardPath = `/boards/${board.slug}`;
    const filteredPath = (changes: Record<string, string | undefined>) => {
      const params = new URLSearchParams();
      for (const [key, value] of Object.entries({ q: query.q, category: query.category, tag: query.tag, sort, ...changes })) {
        if (value) params.set(key, value);
      }
      return buildTenantPath(boardPath, tenant, defaultTenantSlug, params);
    };
    const kind = boardKind(board.board_type);
    const action = kind === "feature-requests" ? "Suggest a feature" : kind === "bug-reports" ? "Report a bug" : "Start a discussion";
    const empty = kind === "announcements" ? "No announcements yet" : kind === "bug-reports" ? "No bug reports yet" : kind === "discussions" ? "No discussions yet" : "No feature requests yet";

    return (
      <div className={`page-stack experience experience--${kind}${board.background_image_url ? " experience--with-background" : ""}`}>
        {presentation.plugins.filter((plugin) => /^\/plugins\/[a-z0-9-]+\.css$/.test(plugin.stylesheet_path)).map((plugin) => <link rel="stylesheet" href={plugin.stylesheet_path} key={plugin.id} />)}
        {board.background_image_url ? <img className="experience__background" src={board.background_image_url} alt="" aria-hidden="true" /> : null}
        <RealtimeRefresh boardId={board.id} tenantSlug={tenant} />
        <section className="page-head">
          <Link className="back-link" href={buildTenantPath("/", tenant, defaultTenantSlug)}>← All boards</Link>
          {board.header_image_url ? <img className="experience__cover" src={board.header_image_url} alt="" /> : null}
          <div className="board-page-heading experience__heading" style={board.background_color ? { backgroundColor: `color-mix(in srgb, ${board.background_color} 22%, white)` } : undefined}>
            <div>
              <span className="experience__eyebrow">{kind === "announcements" ? "Updates" : kind === "bug-reports" ? "Issue tracker" : kind === "discussions" ? "Community" : "Ideas"}</span>
              <h1 className="page-title">{board.name}</h1>
              {board.description ? <p className="page-lead">{board.description}</p> : null}
              {board.intro_text ? <p className="experience__intro">{board.intro_text}</p> : null}
            </div>
            {kind !== "announcements" ? <Link className="button" href={buildTenantPath(`/boards/${board.slug}/new`, tenant, defaultTenantSlug)}>{action}</Link> : null}
          </div>
        </section>

        {presentation.announcement ? <div className="experience__announcement" role="note">{presentation.announcement}</div> : null}

        <form className="experience__filters" action={buildTenantPath(boardPath, tenant, defaultTenantSlug)} method="get" role="search">
          <label className="experience__search"><span className="sr-only">Search topics</span><input name="q" type="search" defaultValue={query.q ?? ""} maxLength={100} placeholder="Search topics in this board" /></label>
          {categories.length ? <label><span className="sr-only">Category</span><select name="category" defaultValue={query.category ?? ""}><option value="">All categories</option>{categories.map((category) => <option value={category.slug} key={category.id}>{category.name}</option>)}</select></label> : null}
          {tags.length ? <label><span className="sr-only">Tag</span><select name="tag" defaultValue={query.tag ?? ""}><option value="">All tags</option>{tags.map((tag) => <option value={tag.slug} key={tag.id}>{tag.name}</option>)}</select></label> : null}
          {sort !== "newest" ? <input type="hidden" name="sort" value={sort} /> : null}
          <button className="button" type="submit">Search</button>
          {query.q || query.category || query.tag ? <Link className="experience__clear" href={buildTenantPath(boardPath, tenant, defaultTenantSlug)}>Clear</Link> : null}
        </form>

        <nav className="experience__sort" aria-label="Sort topics">
          <Link className={sort === "newest" ? "experience__sort-active" : ""} href={filteredPath({ sort: "newest", page: undefined })}>Latest</Link>
          <Link className={sort === "hot" ? "experience__sort-active" : ""} href={filteredPath({ sort: "hot", page: undefined })}>Hot</Link>
          {board.allow_votes ? <Link className={sort === "top" ? "experience__sort-active" : ""} href={filteredPath({ sort: "top", page: undefined })}>Top</Link> : null}
        </nav>

        <div className={`experience__forum${presentation.sidebar_text || categories.length ? " experience__forum--with-sidebar" : ""}`}>
        <div className="experience__forum-main">

        {posts.length > 0 ? (
          <section className="experience__posts" aria-label={`${board.name} posts`}>
            {posts.map((post) => (
              <Link
                className="experience__post"
                href={buildTenantPath(`/posts/${post.id}`, tenant, defaultTenantSlug)}
                key={post.id}
              >
                {kind === "feature-requests" && board.allow_votes ? <span className="experience__votes"><strong>{post.vote_count}</strong><small>votes</small></span> : null}
                {kind === "announcements" ? <time className="experience__date" dateTime={post.created_at}>{new Date(post.created_at).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" })}</time> : null}
                {kind === "bug-reports" ? <span className="experience__bug-mark" aria-hidden="true">!</span> : null}
                <div className="experience__post-main">
                  <strong>{post.title}</strong>
                  {post.is_pinned ? <span className="experience__pinned">Pinned</span> : null}
                  {post.category_name || post.tag_names.length ? <div className="experience__post-labels">
                    {post.category_name ? <span className="experience__category"><i style={{ background: post.category_color ?? "#64748b" }} />{post.category_name}</span> : null}
                    {post.tag_names.map((tagName) => <span className="experience__tag" key={tagName}>#{tagName}</span>)}
                  </div> : null}
                  <p className="section-subtitle">
                    {kind === "bug-reports" ? "Bug report" : kind === "discussions" ? "Discussion" : kind === "announcements" ? "Team update" : "Feature request"}
                    {board.allow_votes && kind !== "feature-requests" ? ` · ${post.vote_count} ${kind === "bug-reports" ? "affected" : kind === "announcements" ? "helpful" : "likes"}` : ""}
                    {board.allow_comments && kind !== "discussions" ? ` · ${countLabel(post.comment_count, "comment")}` : ""}
                    {post.duplicate_of_post_id ? " • duplicate" : ""}
                  </p>
                </div>
                {(kind === "feature-requests" || kind === "bug-reports") ? <StatusPill status={post.status} /> : null}
                {kind === "discussions" ? <span className="experience__reply-count">{post.comment_count} replies</span> : null}
              </Link>
            ))}
          </section>
        ) : (
          <section className="panel empty-state">
            <h2 className="empty-state__title">{query.q || query.category || query.tag ? "No matching topics" : empty}</h2>
            <p className="empty-state__copy">{query.q || query.category || query.tag ? "Try another search or clear the filters." : kind === "announcements" ? "Updates from the team will appear here." : `Be the first to ${kind === "bug-reports" ? "report a problem" : kind === "discussions" ? "start a conversation" : "share an idea"}.`}</p>
          </section>
        )}
        {page > 1 || postsPage.has_next ? <nav className="experience__pagination" aria-label="Topic pages">
          {page > 1 ? <Link href={filteredPath({ page: String(page - 1) })}>← Previous</Link> : <span />}
          <span>Page {page}</span>
          {postsPage.has_next ? <Link href={filteredPath({ page: String(page + 1) })}>Next →</Link> : <span />}
        </nav> : null}
        </div>
        {presentation.sidebar_text || categories.length ? <aside className="experience__sidebar" aria-label="Board information">
          {presentation.sidebar_text ? <section><h2>About this board</h2><p>{presentation.sidebar_text}</p></section> : null}
          {categories.length ? <section><h2>Categories</h2><nav aria-label="Categories"><Link href={filteredPath({ category: undefined, page: undefined })}>All topics</Link>{categories.map((category) => <Link href={filteredPath({ category: category.slug, page: undefined })} key={category.id}>{category.name}</Link>)}</nav></section> : null}
        </aside> : null}
        </div>
        {presentation.footer_text ? <footer className="experience__footer">{presentation.footer_text}</footer> : null}
      </div>
    );
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) notFound();
    return (
      <div className="page-stack">
        <ApiUnavailable
          message={error instanceof Error ? error.message : "Failed to load this board."}
        />
      </div>
    );
  }
}
