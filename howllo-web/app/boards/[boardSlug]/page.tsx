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

    return (
      <div className="page-stack">
        <RealtimeRefresh boardId={board.id} tenantSlug={tenant} />
        <section className="page-head">
          <Link className="back-link" href={buildTenantPath("/", tenant, defaultTenantSlug)}>← All boards</Link>
          <div className="board-page-heading" style={board.background_color ? { backgroundColor: board.background_color } : undefined}>
            <div>
              <h1 className="page-title">{board.name}</h1>
              {board.description ? <p className="page-lead">{board.description}</p> : null}
            </div>
            <Link className="button" href={buildTenantPath(`/boards/${board.slug}/new`, tenant, defaultTenantSlug)}>New post</Link>
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
