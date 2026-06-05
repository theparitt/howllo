import Link from "next/link";
import { getBoardDetail, getBoardPosts } from "@/lib/api";
import { buildTenantPath, resolveTenantContext } from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";
import { ApiUnavailable } from "@/components/api-unavailable";
import { StatusPill } from "@/components/status-pill";

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
  const { tenantSlug: tenant, defaultTenantSlug } = await resolveTenantContext(query.tenant);
  const sort = query.sort ?? "newest";
  const status = query.status;
  const page = Number(query.page ?? "1");
  const token = await getServerBearerToken();

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
        <section className="page-head">
          <Link className="back-link" href={buildTenantPath("/", tenant, defaultTenantSlug)}>
            ← Boards
          </Link>
          <div className="eyebrow-row">
            <span
              className={`board-hero-icon board-hero-icon--sm${board.icon_url ? "" : " board-hero-icon--default"}`}
            >
              <img src={board.icon_url || "/brand/howllo-logo.svg"} alt="" />
            </span>
            <span className="kicker">{board.board_type}</span>
            {board.is_private ? <span className="chip" data-tone="blue">Private</span> : null}
          </div>
          <h1 className="page-title">{board.name}</h1>
          {board.description ? <p className="page-lead">{board.description}</p> : null}
          <div className="hero__actions" style={{ marginTop: "0.4rem" }}>
            <Link
              className="button"
              href={buildTenantPath(`/boards/${board.slug}/new`, tenant, defaultTenantSlug)}
            >
              Submit feedback
            </Link>
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
                    {post.vote_count} votes • {post.comment_count} comments
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
            <p className="empty-state__copy">Be the first to submit feedback for this board.</p>
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
