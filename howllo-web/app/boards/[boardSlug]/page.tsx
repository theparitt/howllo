import Link from "next/link";
import { getBoardDetail, getBoardPosts } from "@/lib/api";
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
  const page = Number(query.page ?? "1");
  const token = await getServerBearerToken(tenant);

  try {
    const [board, posts] = await Promise.all([
      getBoardDetail(tenant, boardSlug, token),
      getBoardPosts({
        tenantSlug: tenant,
        boardSlug,
        sort,
        status,
        page,
        perPage: 20,
        token,
      }),
    ]);
    const kind = boardKind(board.board_type);
    const action = kind === "feature-requests" ? "Suggest a feature" : kind === "bug-reports" ? "Report a bug" : "Start a discussion";
    const empty = kind === "announcements" ? "No announcements yet" : kind === "bug-reports" ? "No bug reports yet" : kind === "discussions" ? "No discussions yet" : "No feature requests yet";

    return (
      <div className={`page-stack experience experience--${kind}`}>
        <RealtimeRefresh boardId={board.id} tenantSlug={tenant} />
        <section className="page-head">
          <Link className="back-link" href={buildTenantPath("/", tenant, defaultTenantSlug)}>← All boards</Link>
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

        {kind === "feature-requests" && board.allow_votes && posts.length > 0 ? <nav className="experience__sort" aria-label="Sort requests">
          <Link className={sort === "top" ? "experience__sort-active" : ""} href={buildTenantPath(`/boards/${board.slug}?sort=top`, tenant, defaultTenantSlug)}>Top voted</Link>
          <Link className={sort !== "top" ? "experience__sort-active" : ""} href={buildTenantPath(`/boards/${board.slug}?sort=newest`, tenant, defaultTenantSlug)}>Newest</Link>
        </nav> : null}

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
            <h2 className="empty-state__title">{empty}</h2>
            <p className="empty-state__copy">{kind === "announcements" ? "Updates from the team will appear here." : `Be the first to ${kind === "bug-reports" ? "report a problem" : kind === "discussions" ? "start a conversation" : "share an idea"}.`}</p>
          </section>
        )}
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
