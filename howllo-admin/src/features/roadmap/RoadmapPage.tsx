import { useEffect, useMemo, useRef, useState } from "react";
import { admin, comments, moderation, posts, roadmap } from "@howllo/api-client";
import type { BoardDetail, Comment, PostDetail, PostStatus, RoadmapItem } from "@howllo/types";
import { SessionRequired } from "../../components/SessionRequired";
import { Panel } from "../../components/Panel";
import { StateBlock } from "../../components/StateBlock";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";
import { humanizeKey } from "../../lib/format";
import { useEscapeKey, useFocusTrap } from "../../lib/a11y";

type RoadmapStatus = "planned" | "in_progress" | "done";

const COLUMNS: RoadmapStatus[] = ["planned", "in_progress", "done"];

const STATUS_META: Record<RoadmapStatus, { tone: "warm" | "blue" | "green"; empty: string }> = {
  planned: {
    tone: "warm",
    empty: "No planned items yet.",
  },
  in_progress: {
    tone: "blue",
    empty: "No work is currently in progress.",
  },
  done: {
    tone: "green",
    empty: "Nothing has been marked done yet.",
  },
};

export function RoadmapPage() {
  const { client, tenant, authorization } = useSession();
  const road = roadmap(client);
  const boardsApi = admin(client);
  const items = useAsync<RoadmapItem[]>(
    () => (authorization ? road.list() : Promise.resolve([])),
    [tenant, authorization],
  );
  const boards = useAsync<BoardDetail[]>(
    () => (authorization ? boardsApi.listBoards() : Promise.resolve([])),
    [tenant, authorization],
  );

  if (!authorization) {
    return (
      <section>
        <h1>Roadmap</h1>
        <SessionRequired />
      </section>
    );
  }

  return (
    <section>
      <h1>Roadmap</h1>
      <p className="muted">
        Create roadmap items here, then move them between Planned, In Progress,
        and Done. Under the hood these are normal posts with roadmap-visible
        statuses.
      </p>

      <CreateRoadmapItem boards={boards.data ?? []} onCreated={items.reload} />

      <StateBlock loading={items.loading || boards.loading} error={items.error ?? boards.error}>
        <div className="roadmap-board">
          {COLUMNS.map((status) => (
            <RoadmapColumn
              key={status}
              status={status}
              items={(items.data ?? []).filter((item) => item.status === status)}
              boards={boards.data ?? []}
              onChanged={items.reload}
            />
          ))}
        </div>
      </StateBlock>
    </section>
  );
}

function CreateRoadmapItem({
  boards,
  onCreated,
}: {
  boards: BoardDetail[];
  onCreated: () => void;
}) {
  const { client } = useSession();
  const postsApi = posts(client);
  const mod = moderation(client);

  const defaultBoardSlug = useMemo(() => {
    return (
      boards.find((board) => board.slug === "feature-requests")?.slug ??
      boards.find((board) => board.slug === "general")?.slug ??
      boards[0]?.slug ??
      ""
    );
  }, [boards]);

  const [boardSlug, setBoardSlug] = useState("");
  const [status, setStatus] = useState<RoadmapStatus>("planned");
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);

  const effectiveBoardSlug = boardSlug || defaultBoardSlug;

  const submit = async () => {
    if (!effectiveBoardSlug) {
      setError("Create a workspace board first.");
      return;
    }
    if (!title.trim()) {
      setError("Title is required.");
      return;
    }
    if (!body.trim()) {
      setError("Description is required.");
      return;
    }

    setBusy(true);
    setError(null);
    setOk(null);
    try {
      const created = await postsApi.create(effectiveBoardSlug, {
        title: title.trim(),
        body: body.trim(),
      });
      await mod.setStatus(created.id, status);
      setOk(`Roadmap item created in ${humanizeKey(status)}.`);
      setTitle("");
      setBody("");
      onCreated();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not create roadmap item.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel title="Create roadmap item">
      <div className="form-grid">
        <label>
          Board
          <select
            value={effectiveBoardSlug}
            onChange={(e) => setBoardSlug(e.target.value)}
            disabled={busy || boards.length === 0}
          >
            {boards.map((board) => (
              <option key={board.id} value={board.slug}>
                {board.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Initial status
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value as RoadmapStatus)}
            disabled={busy}
          >
            {COLUMNS.map((item) => (
              <option key={item} value={item}>
                {humanizeKey(item)}
              </option>
            ))}
          </select>
        </label>
        <label className="field-span-2">
          Title
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Example: Public API for integrations"
          />
        </label>
        <label className="field-span-2">
          Description
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            rows={4}
            placeholder="What is shipping, why it matters, and what the team is planning."
          />
        </label>
      </div>

      {ok ? <p className="storage-ok">{ok}</p> : null}
      {error ? <p className="error-text">{error}</p> : null}

      <div className="button-row" style={{ marginTop: 16 }}>
        <button className="primary" disabled={busy || boards.length === 0} onClick={submit}>
          {busy ? "Creating..." : "Create roadmap item"}
        </button>
      </div>
    </Panel>
  );
}

function RoadmapColumn({
  status,
  items,
  boards,
  onChanged,
}: {
  status: RoadmapStatus;
  items: RoadmapItem[];
  boards: BoardDetail[];
  onChanged: () => void;
}) {
  const meta = STATUS_META[status];

  return (
    <Panel
      title={humanizeKey(status)}
      actions={<span className="badge" data-tone={meta.tone}>{items.length} items</span>}
    >
      {items.length === 0 ? (
        <div className="roadmap-empty">
          <strong>{humanizeKey(status)}</strong>
          <p>{meta.empty}</p>
        </div>
      ) : (
        <ul className="roadmap-card-list">
          {items.map((item) => (
            <RoadmapItemRow
              key={item.id}
              item={item}
              boards={boards}
              onChanged={onChanged}
            />
          ))}
        </ul>
      )}
    </Panel>
  );
}

function RoadmapItemRow({
  item,
  boards,
  onChanged,
}: {
  item: RoadmapItem;
  boards: BoardDetail[];
  onChanged: () => void;
}) {
  const { client } = useSession();
  const mod = moderation(client);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const itemStatus = (COLUMNS.includes(item.status as RoadmapStatus)
    ? item.status
    : "planned") as RoadmapStatus;
  const currentIndex = COLUMNS.indexOf(itemStatus);
  const previousStatus = currentIndex > 0 ? COLUMNS[currentIndex - 1] : null;
  const nextStatus =
    currentIndex >= 0 && currentIndex < COLUMNS.length - 1
      ? COLUMNS[currentIndex + 1]
      : null;

  const move = async (nextStatus: RoadmapStatus) => {
    setBusy(true);
    setError(null);
    try {
      await mod.setStatus(item.id, nextStatus);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not update roadmap item.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <li className="roadmap-card">
      <div className="roadmap-card__meta">
        <span className="badge" data-tone={STATUS_META[itemStatus].tone}>
          {humanizeKey(itemStatus)}
        </span>
      </div>
      <div className="roadmap-item__body">
        <strong className="roadmap-card__title">{item.title}</strong>
        <div className="roadmap-card__stats">
          <span className="badge">{item.vote_count} votes</span>
          <span className="badge">{item.comment_count} comments</span>
        </div>
        {error ? <div className="error-text">{error}</div> : null}
        <div className="roadmap-card__actions">
          {previousStatus ? (
            <button
              type="button"
              className="small"
              disabled={busy}
              onClick={() => void move(previousStatus)}
            >
              Move to {humanizeKey(previousStatus)}
            </button>
          ) : null}
          {nextStatus ? (
            <button
              type="button"
              className="small primary"
              disabled={busy}
              onClick={() => void move(nextStatus)}
            >
              Move to {humanizeKey(nextStatus)}
            </button>
          ) : null}
          <button type="button" onClick={() => setOpen(true)}>
            Open details
          </button>
        </div>
      </div>

      {open ? (
        <RoadmapItemModal
          item={item}
          boards={boards}
          onClose={() => setOpen(false)}
          onChanged={() => {
            setOpen(false);
            onChanged();
          }}
        />
      ) : null}
    </li>
  );
}

function RoadmapItemModal({
  item,
  boards,
  onClose,
  onChanged,
}: {
  item: RoadmapItem;
  boards: BoardDetail[];
  onClose: () => void;
  onChanged: () => void;
}) {
  const { client } = useSession();
  const postsApi = posts(client);
  const commentsApi = comments(client);
  const mod = moderation(client);
  const dialogRef = useRef<HTMLDivElement>(null);

  const detail = useAsync<PostDetail | null>(
    () => postsApi.get(item.id),
    [item.id],
  );
  const thread = useAsync<Comment[]>(
    () => commentsApi.listByPost(item.id),
    [item.id],
  );

  const [title, setTitle] = useState(item.title);
  const [body, setBody] = useState("");
  const [status, setStatus] = useState(item.status);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEscapeKey(true, onClose);
  useFocusTrap(true, dialogRef);

  const boardName =
    boards.find((board) => board.slug === detail.data?.board_slug)?.name ?? "-";

  useEffect(() => {
    if (detail.data?.body) {
      setBody(detail.data.body);
    }
  }, [detail.data?.body]);

  const save = async () => {
    if (!title.trim() || !body.trim()) {
      setError("Title and description are required.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await postsApi.update(item.id, {
        title: title.trim(),
        body: body.trim(),
      });
      await mod.setStatus(item.id, status);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not save roadmap item.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-card modal-card--wide"
        role="dialog"
        aria-modal="true"
        ref={dialogRef}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="modal-title">Roadmap item details</h2>
        <StateBlock loading={detail.loading || thread.loading} error={detail.error ?? thread.error}>
          <div className="detail-stack">
            <div className="detail-list">
              <div>
                <dt>Board</dt>
                <dd>{boardName}</dd>
              </div>
              <div>
                <dt>Votes</dt>
                <dd>{item.vote_count}</dd>
              </div>
              <div>
                <dt>Comments</dt>
                <dd>{item.comment_count}</dd>
              </div>
            </div>

            <div className="form-grid">
              <label className="field-span-2">
                Title
                <input value={title} onChange={(e) => setTitle(e.target.value)} />
              </label>
              <label>
                Status
                <select value={status} onChange={(e) => setStatus(e.target.value as PostStatus)}>
                  {COLUMNS.map((entry) => (
                    <option key={entry} value={entry}>
                      {humanizeKey(entry)}
                    </option>
                  ))}
                </select>
              </label>
              <div />
              <label className="field-span-2">
                Description
                <textarea
                  rows={6}
                  value={body}
                  onChange={(e) => setBody(e.target.value)}
                />
              </label>
            </div>

            <Panel title="Recent comments">
              {thread.data?.length ? (
                <ul className="timeline-list">
                  {thread.data.map((comment) => (
                    <li key={comment.id}>
                      <div>
                        <strong>{comment.display_name}</strong>
                        <div className="muted">{comment.body}</div>
                      </div>
                      <time>{new Date(comment.created_at).toLocaleString()}</time>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="muted">No comments yet.</p>
              )}
            </Panel>

            {error ? <p className="error-text">{error}</p> : null}
          </div>
        </StateBlock>

        <div className="modal-actions">
          <button onClick={onClose} disabled={busy}>
            Close
          </button>
          <button className="primary" onClick={save} disabled={busy}>
            {busy ? "Saving..." : "Save changes"}
          </button>
        </div>
      </div>
    </div>
  );
}
