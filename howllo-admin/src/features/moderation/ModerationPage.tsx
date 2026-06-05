import { useRef, useState } from "react";
import { admin, moderation, posts } from "@howllo/api-client";
import type { BoardDetail, PostDetail, PostListItem, PostStatus } from "@howllo/types";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";
import { Panel } from "../../components/Panel";
import { StateBlock } from "../../components/StateBlock";
import { formatDateTime } from "../../lib/format";
import { useEscapeKey, useFocusTrap } from "../../lib/a11y";

const STATUSES: PostStatus[] = [
  "under_review",
  "planned",
  "in_progress",
  "done",
  "declined",
];

export function ModerationPage() {
  const { client, tenant, authorization } = useSession();
  const boardsApi = admin(client);
  const boards = useAsync<BoardDetail[]>(
    () => (authorization ? boardsApi.listBoards() : Promise.resolve([])),
    [tenant, authorization],
  );
  const [boardSlug, setBoardSlug] = useState<string>("");
  const [includeHidden, setIncludeHidden] = useState(false);

  // Default to the first board once loaded.
  const activeSlug = boardSlug || boards.data?.[0]?.slug || "";

  return (
    <section>
      <h1>Moderation</h1>
      <p className="muted">
        Content operations for admins and moderators. This section is separate
        from workspace management.
      </p>

      <Panel
        title="Board"
        actions={
          <div className="panel-actions">
            <button
              type="button"
              className={includeHidden ? "icon-toggle is-on" : "icon-toggle"}
              aria-pressed={includeHidden}
              title={includeHidden ? "Showing hidden & deleted posts" : "Show hidden & deleted posts"}
              onClick={() => setIncludeHidden((v) => !v)}
            >
              {includeHidden ? (
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                  <circle cx="12" cy="12" r="3" />
                </svg>
              ) : (
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94M9.9 4.24A9.12 9.12 0 0112 4c7 0 11 8 11 8a18.5 18.5 0 01-2.16 3.19m-6.72-1.07a3 3 0 11-4.24-4.24" />
                  <path d="M1 1l22 22" />
                </svg>
              )}
              <span className="icon-toggle__label">Hidden</span>
            </button>
            <select
              value={activeSlug}
              onChange={(e) => setBoardSlug(e.target.value)}
              disabled={!boards.data?.length}
            >
              {boards.data?.map((b) => (
                <option key={b.id} value={b.slug}>
                  {b.name}
                </option>
              ))}
            </select>
          </div>
        }
      >
        <StateBlock
          loading={boards.loading}
          error={boards.error}
          empty={boards.data?.length === 0 ? "No boards to moderate." : null}
        >
          {activeSlug ? (
            <PostQueue boardSlug={activeSlug} includeHidden={includeHidden} />
          ) : (
            <p className="muted">Select a board.</p>
          )}
        </StateBlock>
      </Panel>
    </section>
  );
}

function PostQueue({
  boardSlug,
  includeHidden,
}: {
  boardSlug: string;
  includeHidden: boolean;
}) {
  const { client } = useSession();
  const postsApi = posts(client);
  const list = useAsync<PostListItem[]>(
    () => postsApi.listByBoard(boardSlug, { includeHidden }),
    [boardSlug, includeHidden],
  );

  return (
    <StateBlock
      loading={list.loading}
      error={list.error}
      empty={list.data?.length === 0 ? "No posts on this board." : null}
    >
      <table className="data-table">
        <thead>
          <tr>
            <th>Title</th>
            <th>Status</th>
            <th>Votes</th>
            <th>Comments</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {list.data?.map((p) => (
            <PostRow
              key={p.id}
              post={p}
              siblingPosts={list.data ?? []}
              onChange={list.reload}
            />
          ))}
        </tbody>
      </table>
    </StateBlock>
  );
}

function PostRow({
  post,
  siblingPosts,
  onChange,
}: {
  post: PostListItem;
  siblingPosts: PostListItem[];
  onChange: () => void;
}) {
  const { client } = useSession();
  const mod = moderation(client);
  const postsApi = posts(client);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  const detail = useAsync<PostDetail | null>(
    () => postsApi.get(post.id),
    [post.id],
  );

  const isRemoved = post.is_hidden || post.deleted_at !== null;
  const isLocked = detail.data?.is_locked ?? false;

  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
      onChange();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Action failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <tr className={isRemoved ? "row-removed" : undefined}>
        <td>
          {post.title}
          {post.duplicate_of_post_id ? (
            <span className="badge">duplicate</span>
          ) : null}
          {isLocked ? <span className="badge">locked</span> : null}
          {post.deleted_at ? (
            <span className="badge danger-badge">deleted</span>
          ) : post.is_hidden ? (
            <span className="badge danger-badge">hidden</span>
          ) : null}
          {error ? <div className="error-text">{error}</div> : null}
        </td>
        <td>
          <select
            value={post.status}
            disabled={busy}
            onChange={(e) => run(() => mod.setStatus(post.id, e.target.value))}
          >
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </select>
        </td>
        <td>{post.vote_count}</td>
        <td>{post.comment_count}</td>
        <td className="row-actions">
          {isRemoved ? (
            <button disabled={busy} onClick={() => run(() => mod.restore(post.id))}>
              Restore
            </button>
          ) : (
            <>
              <button
                disabled={busy}
                onClick={() => run(() => mod.setVisibility(post.id, true))}
              >
                Hide
              </button>
              <button
                disabled={busy}
                onClick={() => run(() => mod.setLock(post.id, !isLocked))}
              >
                {detail.loading ? "Loading..." : isLocked ? "Unlock" : "Lock"}
              </button>
              <button disabled={busy} onClick={() => setDuplicateOpen(true)}>
                {post.duplicate_of_post_id ? "Change duplicate" : "Mark duplicate"}
              </button>
              {post.duplicate_of_post_id ? (
                <button
                  disabled={busy}
                  onClick={() => run(() => mod.setDuplicate(post.id, null))}
                >
                  Clear duplicate
                </button>
              ) : null}
              <button
                disabled={busy}
                onClick={() => void detail.reload()}
              >
                Refresh state
              </button>
              <button
                disabled={busy}
                className="danger"
                onClick={() => run(() => mod.softDelete(post.id))}
              >
                Delete
              </button>
            </>
          )}
        </td>
      </tr>

      {duplicateOpen ? (
        <DuplicateModal
          post={post}
          posts={siblingPosts}
          onClose={() => setDuplicateOpen(false)}
          onSubmit={(duplicateOfPostId) =>
            run(() => mod.setDuplicate(post.id, duplicateOfPostId)).finally(() => {
              setDuplicateOpen(false);
            })
          }
        />
      ) : null}
    </>
  );
}

function DuplicateModal({
  post,
  posts,
  onClose,
  onSubmit,
}: {
  post: PostListItem;
  posts: PostListItem[];
  onClose: () => void;
  onSubmit: (duplicateOfPostId: string | null) => Promise<void>;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const [selectedId, setSelectedId] = useState(post.duplicate_of_post_id ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEscapeKey(true, onClose);
  useFocusTrap(true, dialogRef);

  const candidates = posts.filter(
    (item) => item.id !== post.id && !item.is_hidden && item.deleted_at === null,
  );

  const save = async () => {
    if (!selectedId) {
      setError("Choose a canonical post.");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      await onSubmit(selectedId);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to update duplicate.");
      setBusy(false);
    }
  };

  const clear = async () => {
    setBusy(true);
    setError(null);
    try {
      await onSubmit(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to clear duplicate.");
      setBusy(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-card"
        role="dialog"
        aria-modal="true"
        ref={dialogRef}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="modal-title">Duplicate management</h2>
        <p className="muted">
          Mark <strong>{post.title}</strong> as a duplicate of another post on this board.
        </p>

        <label className="auth-field" style={{ marginTop: 4 }}>
          <span>Canonical post</span>
          <select
            value={selectedId}
            onChange={(e) => setSelectedId(e.target.value)}
            disabled={busy || candidates.length === 0}
          >
            <option value="">Select a post</option>
            {candidates.map((candidate) => (
              <option key={candidate.id} value={candidate.id}>
                {candidate.title} ({formatDateTime(candidate.created_at)})
              </option>
            ))}
          </select>
        </label>

        {candidates.length === 0 ? (
          <p className="muted">No eligible posts are available on this board.</p>
        ) : null}
        {error ? <p className="error-text">{error}</p> : null}

        <div className="modal-actions">
          <button onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button onClick={clear} disabled={busy || !post.duplicate_of_post_id}>
            Clear
          </button>
          <button className="primary" onClick={save} disabled={busy || !selectedId}>
            {busy ? "Saving..." : "Save duplicate"}
          </button>
        </div>
      </div>
    </div>
  );
}
