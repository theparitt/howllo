import Link from "next/link";
import { notFound } from "next/navigation";
import { ApiError, getBoardDetail, getComments, getPostDetail, getStatusHistory } from "@/lib/api";
import {
  WorkspaceContextError,
  buildTenantPath,
  resolveTenantContext,
} from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";
import { StatusPill } from "@/components/status-pill";
import { CommentComposer, PostActions } from "@/components/post-actions";
import { ModerationPanel } from "@/components/moderation-panel";
import { RealtimeRefresh } from "@/components/realtime-refresh";
import { WorkspaceState } from "@/components/workspace-state";
import { plural } from "@/lib/format";
import { boardKind, boardVoteLabel } from "@/lib/board-experience";

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
    const board = await getBoardDetail(tenant, post.board_slug, token);
    const kind = boardKind(board.board_type);
    const bugSections = kind === "bug-reports" && post.body.startsWith("Steps to reproduce\n")
      ? post.body.split(/\n\n(?=(?:Expected result|Actual result|Environment)\n)/).map((part) => {
          const split = part.indexOf("\n");
          return { label: part.slice(0, split), content: part.slice(split + 1) };
        })
      : null;

    return (
      <div className={`post-thread post-thread--${kind}`}>
        <RealtimeRefresh postId={post.id} tenantSlug={tenant} />
          <section className="post-detail">
            <div className="eyebrow-row">
              <Link
                className="muted"
                href={buildTenantPath(`/boards/${post.board_slug}`, tenant, defaultTenantSlug)}
              >
                ← Back to board
              </Link>
              <span className="chip">{board.name}</span>
            </div>
            <div className="toolbar" style={{ justifyContent: "space-between", alignItems: "flex-start", marginTop: "1rem" }}>
              <div className="stack stack--tight">
                <h1 className="page-title" style={{ fontSize: "2rem" }}>
                  {post.title}
                </h1>
                <div className="metric-row">
                  {board.allow_votes ? <span><strong>{post.vote_count}</strong> {plural(post.vote_count, "vote")}</span> : null}
                  {board.allow_comments ? <span><strong>{comments.length}</strong> {plural(comments.length, "comment")}</span> : null}
                  <span>{new Date(post.created_at).toLocaleDateString()}</span>
                </div>
              </div>
              {kind === "feature-requests" || kind === "bug-reports" ? <StatusPill status={post.status} /> : null}
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
            {bugSections ? <div className="bug-report-sections">{bugSections.map((section) => <section key={section.label}><h2>{section.label}</h2><p className="post-body">{section.content}</p></section>)}</div> : <p className="post-body">{post.body}</p>}
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
            <PostActions tenantSlug={tenant} postId={post.id} allowVotes={board.allow_votes} voteLabel={boardVoteLabel(board.board_type)} />
          </section>

          {board.allow_comments ? <section className="panel post-thread__comments" id="comments">
            <h2 className="section-title" style={{ fontSize: "1.15rem" }}>{kind === "discussions" ? "Replies" : kind === "announcements" ? "Responses" : "Comments"} ({comments.length})</h2>
            <div className="post-thread__comment-list">
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
                <p className="muted">No comments yet.</p>
              )}
            </div>
            <CommentComposer tenantSlug={tenant} isLocked={post.is_locked} postId={post.id} />
          </section> : null}
          {history.length > 0 ? (
            <details className="post-history">
              <summary>Status history</summary>
              <ol>
                {history.map((item) => (
                  <li key={item.id}>
                    <span>{item.old_status ?? "none"} → {item.new_status}</span>
                    <span className="muted">{new Date(item.created_at).toLocaleString()}</span>
                    {item.reason ? <span className="muted">{item.reason}</span> : null}
                  </li>
                ))}
              </ol>
            </details>
          ) : null}
          <ModerationPanel
            tenantSlug={tenant}
            boardSlug={post.board_slug}
            postId={post.id}
            currentStatus={post.status}
            isLocked={post.is_locked}
          />
      </div>
    );
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) {
      notFound();
    }

    return (
      <div className="notice notice--error">
        {error instanceof Error ? error.message : "Failed to load the post."}
      </div>
    );
  }
}
