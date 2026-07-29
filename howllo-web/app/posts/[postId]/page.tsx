import Link from "next/link";
import { notFound } from "next/navigation";
import { getComments, getPostDetail, getStatusHistory } from "@/lib/api";
import {
  WorkspaceContextError,
  buildTenantPath,
  resolveTenantContext,
} from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";
import { StatusPill } from "@/components/status-pill";
import { PostActions } from "@/components/post-actions";
import { RealtimeRefresh } from "@/components/realtime-refresh";
import { WorkspaceState } from "@/components/workspace-state";

type PostPageProps = {
  params: Promise<{
    postId: string;
  }>;
  searchParams: Promise<{
    tenant?: string;
  }>;
};

export default async function PostPage({ params, searchParams }: PostPageProps) {
  const { postId } = await params;
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

  const token = await getServerBearerToken(tenant);

  try {
    const [post, comments, history] = await Promise.all([
      getPostDetail(tenant, postId, token),
      getComments(postId, token),
      getStatusHistory(tenant, postId, token),
    ]);

    return (
      <div className="two-column">
        <RealtimeRefresh postId={post.id} tenantSlug={tenant} />
        <div className="grid" style={{ gap: "1rem" }}>
          <section className="post-detail">
            <div className="eyebrow-row">
              <Link
                className="muted"
                href={buildTenantPath(`/boards/${post.board_slug}`, tenant, defaultTenantSlug)}
              >
                ← Back to board
              </Link>
              <span className="chip">{post.board_slug}</span>
            </div>
            <div className="toolbar" style={{ justifyContent: "space-between", alignItems: "flex-start", marginTop: "1rem" }}>
              <div className="stack stack--tight">
                <h1 className="page-title" style={{ fontSize: "2rem" }}>
                  {post.title}
                </h1>
                <div className="metric-row">
                  <span><strong>{post.vote_count}</strong> votes</span>
                  <span><strong>{comments.length}</strong> comments</span>
                  <span>{new Date(post.created_at).toLocaleDateString()}</span>
                </div>
              </div>
              <StatusPill status={post.status} />
            </div>
            {post.duplicate_of_post_id ? (
              <div className="notice" style={{ marginTop: "1rem" }}>
                Marked as a duplicate of another request.
              </div>
            ) : null}
            {post.is_locked ? (
              <div className="notice" style={{ marginTop: "1rem" }}>
                This conversation is locked.
              </div>
            ) : null}
            <hr className="divider" />
            <p className="post-body">{post.body}</p>
            {post.attachments && post.attachments.length > 0 ? (
              <div className="attachment-grid" style={{ marginTop: "1.1rem" }}>
                {post.attachments.map((url) => (
                  <a
                    className="attachment-thumb"
                    href={url}
                    target="_blank"
                    rel="noreferrer"
                    key={url}
                  >
                    <img src={url} alt="Attachment" />
                  </a>
                ))}
              </div>
            ) : null}
          </section>

          <section className="panel">
            <h2 className="section-title" style={{ fontSize: "1.15rem" }}>History</h2>
            <div className="grid" style={{ marginTop: "1rem" }}>
              {history.length > 0 ? history.map((item) => (
                <div className="comment-card" key={item.id}>
                  <div className="toolbar" style={{ justifyContent: "space-between" }}>
                    <div className="chip">
                      {item.old_status ?? "none"} → {item.new_status}
                    </div>
                    <span className="muted" style={{ fontSize: "0.82rem" }}>
                      {new Date(item.created_at).toLocaleString()}
                    </span>
                  </div>
                  <p className="muted" style={{ marginBottom: 0 }}>
                    {item.actor_display_name}
                    {item.reason ? ` • ${item.reason}` : ""}
                  </p>
                </div>
              )) : (
                <div className="empty-state">
                  <h3 className="empty-state__title">No status changes yet</h3>
                </div>
              )}
            </div>
          </section>

          <section className="panel">
            <h2 className="section-title" style={{ fontSize: "1.15rem" }}>Comments</h2>
            <div className="grid" style={{ marginTop: "1rem" }}>
              {comments.length > 0 ? comments.map((comment) => (
                <article className="comment-card" key={comment.id}>
                  <div className="toolbar" style={{ justifyContent: "space-between" }}>
                    <strong>{comment.display_name}</strong>
                    <div className="toolbar">
                      {comment.comment_type !== "user" ? (
                        <span className="chip">{comment.comment_type}</span>
                      ) : null}
                      <span className="muted" style={{ fontSize: "0.82rem" }}>
                        {new Date(comment.created_at).toLocaleString()}
                      </span>
                    </div>
                  </div>
                  <p className="section-subtitle" style={{ color: "var(--text)", whiteSpace: "pre-wrap" }}>
                    {comment.body}
                  </p>
                </article>
              )) : (
                <div className="empty-state">
                  <h3 className="empty-state__title">No comments yet</h3>
                  <p className="empty-state__copy">Be the first to add context.</p>
                </div>
              )}
            </div>
          </section>
        </div>

        <div className="grid" style={{ gap: "1rem" }}>
          <PostActions tenantSlug={tenant} isLocked={post.is_locked} postId={post.id} />
        </div>
      </div>
    );
  } catch (error) {
    if (error instanceof Error && error.message.includes("404")) {
      notFound();
    }

    return (
      <div className="notice notice--error">
        {error instanceof Error ? error.message : "Failed to load the post."}
      </div>
    );
  }
}
