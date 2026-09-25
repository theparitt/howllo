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
import { themedSurfaceStyle } from "@/lib/theme";
import { countLabel } from "@/lib/format";

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
      getBoardDetail(tenant, boardSlug, token).catch((error) => {
        throw new Error(
          error instanceof Error ? error.message : "Failed to load board detail.",
        );
      }),
      getBoardPosts({
        tenantSlug: tenant,
        boardSlug,
        sort,
        status,
        page,
        perPage: 20,
        token,
      }).catch((error) => {
        throw new Error(
          error instanceof Error ? error.message : "Failed to load board posts.",
        );
      }),
    ]);

    return (
      <div className="page-stack">
        <RealtimeRefresh boardId={board.id} tenantSlug={tenant} />
        <section
          className="hero board-hero board-themed-surface"
          style={themedSurfaceStyle(board.background_color)}
        >
          <div className="board-hero__topline">
            <Link className="back-link" href={buildTenantPath("/", tenant, defaultTenantSlug)}>
              ← Boards
            </Link>
            <div className="board-hero__chips">
              <span className="chip">{posts.length} posts</span>
              {board.is_private ? <span className="chip" data-tone="blue">Private</span> : null}
            </div>
          </div>
          <div className="board-hero__body">
            <div className="board-hero__identity">
              <span
                className={`board-hero-icon${board.icon_url ? "" : " board-hero-icon--default"}`}
              >
                {board.icon_url ? <img src={board.icon_url} alt="" /> : <span className="board-hero-icon__letter">{board.name.trim().charAt(0).toUpperCase()}</span>}
              </span>
              <div className="stack stack--tight">
                <div className="eyebrow-row">
                  <span className="kicker">{board.board_type}</span>
                </div>
                <h1 className="page-title">{board.name}</h1>
                {board.description ? (
                  <p className="page-lead">{board.description}</p>
                ) : (
                  <p className="page-lead">
                    Ideas, requests, and issues for this board live here.
                  </p>
                )}
              </div>
            </div>
            <div className="board-hero__actions">
              <Link
                className="button button--cta"
                href={buildTenantPath(`/boards/${board.slug}/new`, tenant, defaultTenantSlug)}
              >
                New post
              </Link>
              <Link
                className="ghost-button"
                href={buildTenantPath("/dashboard", tenant, defaultTenantSlug, new URLSearchParams({
                  board: board.slug,
                }))}
              >
                Board overview
              </Link>
            </div>
          </div>
        </section>

        {posts.length > 0 ? (
          <section className="list-stack">
            {posts.map((post) => (
              <Link
                className="list-row"
                href={buildTenantPath(`/posts/${post.id}`, tenant, defaultTenantSlug)}
                key={post.id}
              >
                <div>
                  <strong>{post.title}</strong>
                  <p className="section-subtitle">
                    {countLabel(post.vote_count, "vote")} • {countLabel(post.comment_count, "comment")}
                    {post.duplicate_of_post_id ? " • duplicate" : ""}
                  </p>
                </div>
                <StatusPill status={post.status} />
              </Link>
            ))}
          </section>
        ) : (
          <section className="panel empty-state">
            <h2 className="empty-state__title">No requests yet</h2>
            <p className="empty-state__copy">Be the first to create a post for this board.</p>
          </section>
        )}
      </div>
    );
  } catch (error) {
    return (
      <div className="page-stack">
        <ApiUnavailable
          message={error instanceof Error ? error.message : "Failed to load this board."}
        />
      </div>
    );
  }
}
