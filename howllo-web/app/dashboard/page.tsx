import Link from "next/link";
import { getBoardPosts, getBoards } from "@/lib/api";
import { buildTenantPath, resolveTenantContext } from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";
import type { Board, PostListItem } from "@/lib/types";
import { ApiUnavailable } from "@/components/api-unavailable";
import { StatusPill } from "@/components/status-pill";

type DashboardPageProps = {
  searchParams: Promise<{
    tenant?: string;
    board?: string;
  }>;
};

function sortByVotes(items: PostListItem[]) {
  return [...items].sort((left, right) => right.vote_count - left.vote_count);
}

function filterRoadmap(items: PostListItem[]) {
  return items.filter((item) =>
    item.status === "planned" || item.status === "in_progress" || item.status === "done",
  );
}

export default async function DashboardPage({ searchParams }: DashboardPageProps) {
  const params = await searchParams;
  const { tenantSlug: tenant, defaultTenantSlug } = await resolveTenantContext(params.tenant);
  const token = await getServerBearerToken();
  let boards: Board[];

  try {
    boards = await getBoards(tenant);
  } catch (error) {
    return (
      <div className="page-stack">
        <ApiUnavailable
          message={error instanceof Error ? error.message : "Failed to load boards."}
        />
      </div>
    );
  }

  const selectedBoard =
    boards.find((board) => board.slug === params.board) ?? boards[0] ?? null;

  if (!selectedBoard) {
    return (
      <div className="page-stack">
        <section className="panel empty-state">
          <h1 className="empty-state__title">No boards are available for this tenant.</h1>
          <p className="empty-state__copy">
            Create a board first, then the app can open directly into feedback for that software or topic.
          </p>
        </section>
      </div>
    );
  }

  const [latestPosts, activePosts] = await Promise.all([
    getBoardPosts({
      tenantSlug: tenant,
      boardSlug: selectedBoard.slug,
      sort: "newest",
      page: 1,
      perPage: 12,
      token,
    }).catch(() => []),
    getBoardPosts({
      tenantSlug: tenant,
      boardSlug: selectedBoard.slug,
      sort: "top",
      page: 1,
      perPage: 12,
      token,
    }).catch(() => []),
  ]);

  const topPosts = sortByVotes(activePosts).slice(0, 5);
  const roadmapItems = filterRoadmap(latestPosts).slice(0, 6);

  return (
    <div className="page-stack">
      <section className="board-bar">
        {boards.map((board: Board) => {
          const isActive = board.slug === selectedBoard.slug;

          return (
            <Link
              className={`board-tab${isActive ? " board-tab--active" : ""}`}
              href={buildTenantPath("/dashboard", tenant, defaultTenantSlug, new URLSearchParams({
                board: board.slug,
              }))}
              key={board.id}
            >
              {board.name}
            </Link>
          );
        })}
      </section>

      <section className="hero hero--with-icon">
        <span
          className={`board-hero-icon${selectedBoard.icon_url ? "" : " board-hero-icon--default"}`}
        >
          <img src={selectedBoard.icon_url || "/brand/howllo-logo.svg"} alt="" />
        </span>
        <div className="stack stack--tight">
          <span className="kicker">{selectedBoard.board_type}</span>
          <h1 className="page-title">{selectedBoard.name}</h1>
          {selectedBoard.description ? (
            <p className="page-lead">{selectedBoard.description}</p>
          ) : null}
        </div>
        <div className="hero__actions">
          <Link
            className="button"
            href={buildTenantPath(`/boards/${selectedBoard.slug}/new`, tenant, defaultTenantSlug)}
          >
            Submit feedback
          </Link>
          <Link
            className="ghost-button"
            href={buildTenantPath(`/boards/${selectedBoard.slug}`, tenant, defaultTenantSlug)}
          >
            View all
          </Link>
        </div>
      </section>

      <section className="dashboard-grid">
        <section className="panel">
          <div className="stack stack--tight">
            <h2 className="section-title">Latest</h2>
          </div>
          <div className="list-stack" style={{ marginTop: "1rem" }}>
            {latestPosts.length > 0 ? (
              latestPosts.map((post) => (
                <Link
                  className="list-row"
                  href={buildTenantPath(`/posts/${post.id}`, tenant, defaultTenantSlug)}
                  key={post.id}
                >
                  <div>
                    <strong>{post.title}</strong>
                    <p className="section-subtitle">
                      {post.comment_count} comments • {post.vote_count} votes
                    </p>
                  </div>
                  <StatusPill status={post.status} />
                </Link>
              ))
            ) : (
              <div className="empty-state">
                <h3 className="empty-state__title">No feedback yet</h3>
                <p className="empty-state__copy">New requests will show up here.</p>
              </div>
            )}
          </div>
        </section>

        <section className="panel">
          <div className="stack stack--tight">
            <h2 className="section-title">Top requests</h2>
          </div>
          <div className="list-stack" style={{ marginTop: "1rem" }}>
            {topPosts.length > 0 ? (
              topPosts.map((post) => (
                <Link
                  className="list-row"
                  href={buildTenantPath(`/posts/${post.id}`, tenant, defaultTenantSlug)}
                  key={post.id}
                >
                  <div>
                    <strong>{post.title}</strong>
                    <p className="section-subtitle">
                      {post.vote_count} votes • {post.comment_count} comments
                    </p>
                  </div>
                  <StatusPill status={post.status} />
                </Link>
              ))
            ) : (
              <div className="empty-state">
                <h3 className="empty-state__title">No votes yet</h3>
                <p className="empty-state__copy">Top-voted requests will surface here.</p>
              </div>
            )}
          </div>
        </section>
      </section>

      <section className="panel">
        <div className="stack stack--tight">
          <h2 className="section-title">Progress</h2>
        </div>
        <div className="status-columns" style={{ marginTop: "1rem" }}>
          <div className="status-column">
            <div className="status-column__header">
              <StatusPill status="planned" />
              <strong>{roadmapItems.filter((item) => item.status === "planned").length}</strong>
            </div>
            {roadmapItems
              .filter((item) => item.status === "planned")
              .map((item) => (
                <Link
                  className="status-column__item"
                  href={buildTenantPath(`/posts/${item.id}`, tenant, defaultTenantSlug)}
                  key={item.id}
                >
                  {item.title}
                </Link>
              ))}
          </div>
          <div className="status-column">
            <div className="status-column__header">
              <StatusPill status="in_progress" />
              <strong>{roadmapItems.filter((item) => item.status === "in_progress").length}</strong>
            </div>
            {roadmapItems
              .filter((item) => item.status === "in_progress")
              .map((item) => (
                <Link
                  className="status-column__item"
                  href={buildTenantPath(`/posts/${item.id}`, tenant, defaultTenantSlug)}
                  key={item.id}
                >
                  {item.title}
                </Link>
              ))}
          </div>
          <div className="status-column">
            <div className="status-column__header">
              <StatusPill status="done" />
              <strong>{roadmapItems.filter((item) => item.status === "done").length}</strong>
            </div>
            {roadmapItems
              .filter((item) => item.status === "done")
              .map((item) => (
                <Link
                  className="status-column__item"
                  href={buildTenantPath(`/posts/${item.id}`, tenant, defaultTenantSlug)}
                  key={item.id}
                >
                  {item.title}
                </Link>
              ))}
          </div>
        </div>
      </section>
    </div>
  );
}
