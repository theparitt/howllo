"use client";

import { useEffect, useMemo, useState } from "react";
import {
  getStatusHistory,
  managerModerationPosts,
  managerModerationQueuePage,
  moderateLock,
  moderatePin,
  moderateReview,
  moderateStatus,
  moderateVisibility,
  postOfficialResponse,
} from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import type { ModerationQueueItem, StatusHistoryItem } from "@/lib/types";

type View = "review" | "posts";

const STATUS_TRANSITIONS: Record<string, string[]> = {
  under_review: ["planned", "declined"],
  planned: ["in_progress", "declined"],
  in_progress: ["done", "planned"],
  done: ["in_progress"],
  declined: ["under_review"],
};

function label(value: string) {
  return value.replaceAll("_", " ");
}

export function ModerationTab({ tenant }: { tenant: string }) {
  const [view, setView] = useState<View>("review");
  const [page, setPage] = useState(1);
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");
  const [items, setItems] = useState<ModerationQueueItem[]>([]);
  const [total, setTotal] = useState(0);
  const [hasNext, setHasNext] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [history, setHistory] = useState<StatusHistoryItem[]>([]);
  const [reason, setReason] = useState("");
  const [official, setOfficial] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [refresh, setRefresh] = useState(0);
  const selected = useMemo(() => items.find((item) => item.id === selectedId) ?? null, [items, selectedId]);

  useEffect(() => {
    if (!selectedId) { setHistory([]); return; }
    let cancelled = false;
    getStatusHistory(tenant, selectedId, readStoredBearerToken(tenant).trim())
      .then((entries) => { if (!cancelled) setHistory(entries); })
      .catch(() => { if (!cancelled) setHistory([]); });
    return () => { cancelled = true; };
  }, [tenant, selectedId, refresh]);

  useEffect(() => {
    let cancelled = false;
    const token = readStoredBearerToken(tenant).trim();
    setLoading(true);
    setError("");
    const request = view === "review"
      ? managerModerationQueuePage(tenant, token, page)
      : managerModerationPosts(tenant, token, page, search);
    request.then((result) => {
      if (cancelled) return;
      setItems(result.items);
      setTotal(result.total);
      setHasNext(result.has_next);
      setSelectedId((current) => current && !result.items.some((item) => item.id === current) ? null : current);
    }).catch((cause) => {
      if (cancelled) return;
      setItems([]);
      setError(cause instanceof Error ? cause.message : "Could not load posts.");
    }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [tenant, view, page, search, refresh]);

  const selectView = (next: View) => {
    if (view === next) return;
    setView(next);
    setPage(1);
    setSelectedId(null);
    setReason("");
    setOfficial("");
    setMessage("");
  };

  const selectPost = (item: ModerationQueueItem) => {
    setSelectedId(item.id);
    setReason("");
    setOfficial("");
    setMessage("");
    setError("");
  };

  const run = async (key: string, task: (token: string) => Promise<void>, success: string, close = false): Promise<boolean> => {
    if (busy) return false;
    setBusy(key);
    setError("");
    setMessage("");
    try {
      await task(readStoredBearerToken(tenant).trim());
      if (close) setSelectedId(null);
      setMessage(success);
      setRefresh((current) => current + 1);
      return true;
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not update this post.");
      return false;
    } finally {
      setBusy(null);
    }
  };

  const review = (action: "approve" | "reject") => {
    if (!selected) return;
    if (action === "reject" && !window.confirm("Reject this post? The author will not see it on the public board.")) return;
    void run(
      action,
      (token) => moderateReview(selected.id, action, token),
      action === "approve" ? "Post approved." : "Post rejected.",
      view === "review",
    );
  };

  const changeStatus = (status: string) => {
    if (!selected) return;
    void run(
      `status-${status}`,
      (token) => moderateStatus(selected.id, status, reason, token),
      `Status changed to ${label(status)}.`,
    ).then((success) => { if (success) setReason(""); });
  };

  const canActOnPublished = selected?.review_state === "approved";
  const nextStatuses = selected ? STATUS_TRANSITIONS[selected.status] ?? [] : [];

  return <section className="panel moderation-browser" aria-label="Moderation">
    <div className="moderation-browser__top">
      <div>
        <h2 className="section-title">Posts</h2>
        <p className="section-subtitle">Review submissions and manage posts in this workspace.</p>
      </div>
      <div className="moderation-browser__tabs" role="group" aria-label="Post list">
        <button className={view === "review" ? "ghost-button moderation-browser__tab--active" : "ghost-button"} type="button" aria-pressed={view === "review"} onClick={() => selectView("review")}>Needs review</button>
        <button className={view === "posts" ? "ghost-button moderation-browser__tab--active" : "ghost-button"} type="button" aria-pressed={view === "posts"} onClick={() => selectView("posts")}>All posts</button>
      </div>
    </div>

    {view === "posts" ? <form className="moderation-browser__search" onSubmit={(event) => { event.preventDefault(); setPage(1); setSelectedId(null); setSearch(searchInput.trim()); }}>
      <label htmlFor="moderation-search">Search posts</label>
      <input id="moderation-search" className="manage-input" type="search" value={searchInput} onChange={(event) => setSearchInput(event.target.value)} placeholder="Title or post text" />
      <button className="ghost-button" type="submit">Search</button>
      {search ? <button className="ghost-button" type="button" onClick={() => { setSearchInput(""); setSearch(""); setPage(1); setSelectedId(null); }}>Clear</button> : null}
    </form> : null}

    {message ? <p className="moderation-browser__success" role="status">{message}</p> : null}
    {error ? <p className="error-text" role="alert">{error}</p> : null}

    <div className="moderation-browser__layout">
      <div className="moderation-browser__list">
        <p className="section-subtitle">{loading ? "Loading posts…" : view === "review" ? `${total} needing review` : `${total} ${total === 1 ? "post" : "posts"}`}</p>
        {!loading && items.length === 0 && !error ? <p className="section-subtitle">{view === "review" ? "No posts need review." : "No posts match your search."}</p> : null}
        {items.map((item) => <button key={item.id} type="button" className={selectedId === item.id ? "moderation-browser__row moderation-browser__row--active" : "moderation-browser__row"} aria-pressed={selectedId === item.id} onClick={() => selectPost(item)}>
          <strong>{item.title}</strong>
          <span>{item.board_slug} · {item.author_display_name}</span>
          <span>{item.review_state === "pending" ? "Waiting for approval" : item.is_hidden ? "Hidden" : label(item.status)} · {new Date(item.created_at).toLocaleDateString()}</span>
        </button>)}
        <div className="list-pager">
          <button className="ghost-button" type="button" disabled={page <= 1 || loading} onClick={() => { setPage((current) => current - 1); setSelectedId(null); }}>Previous</button>
          <span>Page {page}</span>
          <button className="ghost-button" type="button" disabled={!hasNext || loading} onClick={() => { setPage((current) => current + 1); setSelectedId(null); }}>Next</button>
        </div>
      </div>

      {selected ? <div className="moderation-browser__detail" aria-label={`Manage ${selected.title}`}>
        <div className="moderation-browser__detail-heading">
          <h3>{selected.title}</h3>
          <button className="ghost-button" type="button" aria-label="Close post details" onClick={() => setSelectedId(null)}>Close</button>
        </div>
        <p className="section-subtitle">{selected.board_slug} · {selected.author_display_name} · {new Date(selected.created_at).toLocaleString()}</p>
        <p className="moderation-browser__body">{selected.body}</p>
        <p className="section-subtitle">Review: {selected.review_state === "pending" ? "Waiting for approval" : label(selected.review_state)}{selected.review_reason && selected.review_state === "pending" ? ` · ${selected.review_reason}` : ""}</p>

        {selected.review_state === "pending" ? <div className="moderation-browser__actions">
          <button className="button button--cta" type="button" disabled={Boolean(busy)} onClick={() => review("approve")}>{busy === "approve" ? "Approving…" : "Approve"}</button>
          <button className="ghost-button" type="button" disabled={Boolean(busy)} onClick={() => review("reject")}>{busy === "reject" ? "Rejecting…" : "Reject"}</button>
        </div> : null}

        {canActOnPublished ? <>
          <div className="moderation-browser__section">
            <div className="moderation-browser__label-row">
              <h4>Status: {label(selected.status)}</h4>
              <details className="staff-invitations__help"><summary aria-label="About status notes">?</summary><p>Status notes appear in the post history. A reason is required when declining a post.</p></details>
            </div>
            {nextStatuses.length > 0 ? <>
              <label htmlFor="moderation-reason">Status note{nextStatuses.includes("declined") ? " (required to decline)" : " (optional)"}</label>
              <input id="moderation-reason" className="manage-input" value={reason} onChange={(event) => setReason(event.target.value)} maxLength={1000} placeholder="Why is this status changing?" />
              <div className="moderation-browser__actions">{nextStatuses.map((status) => <button key={status} className="ghost-button" type="button" disabled={Boolean(busy) || (status === "declined" && !reason.trim())} title={status === "declined" && !reason.trim() ? "Add a reason to decline this post" : undefined} onClick={() => changeStatus(status)}>{busy === `status-${status}` ? "Saving…" : `Move to ${label(status)}`}</button>)}</div>
            </> : null}
          </div>

          <div className="moderation-browser__section">
            <h4>Official reply</h4>
            <p className="section-subtitle">A public update below the post. Followers may be notified.</p>
            <label htmlFor="moderation-official">Reply text</label>
            <textarea id="moderation-official" className="manage-input" rows={4} value={official} onChange={(event) => setOfficial(event.target.value)} maxLength={10000} />
            <button className="button button--cta" type="button" disabled={Boolean(busy) || !official.trim()} onClick={() => { void run("official", (token) => postOfficialResponse(selected.id, official.trim(), token), "Official reply posted.").then((success) => { if (success) setOfficial(""); }); }}>{busy === "official" ? "Posting…" : "Post official reply"}</button>
          </div>

          {history.length > 0 ? <details className="moderation-browser__history">
            <summary>Status history</summary>
            <ol>{history.map((entry) => <li key={entry.id}><strong>{label(entry.new_status)}</strong>{entry.reason ? ` · ${entry.reason}` : ""}<span> · {entry.actor_display_name} · {new Date(entry.created_at).toLocaleString()}</span></li>)}</ol>
          </details> : null}

          <div className="moderation-browser__section">
            <h4>Post controls</h4>
            <div className="moderation-browser__actions">
              <button className="ghost-button" type="button" disabled={Boolean(busy)} onClick={() => void run("pin", (token) => moderatePin(selected.id, !selected.is_pinned, token), selected.is_pinned ? "Post unpinned." : "Post pinned.")}>{selected.is_pinned ? "Unpin" : "Pin"}</button>
              <button className="ghost-button" type="button" disabled={Boolean(busy)} onClick={() => void run("lock", (token) => moderateLock(selected.id, !selected.is_locked, token), selected.is_locked ? "Post unlocked." : "Post locked.")}>{selected.is_locked ? "Unlock" : "Lock"}</button>
              <button className="ghost-button" type="button" disabled={Boolean(busy)} onClick={() => {
                if (!selected.is_hidden && !window.confirm("Hide this post from the public board?")) return;
                void run("visibility", (token) => moderateVisibility(selected.id, !selected.is_hidden, token), selected.is_hidden ? "Post shown." : "Post hidden.", view === "review");
              }}>{selected.is_hidden ? "Show" : "Hide"}</button>
            </div>
          </div>
        </> : null}
      </div> : <div className="moderation-browser__placeholder"><p>Select a post to review its content and actions.</p></div>}
    </div>
  </section>;
}
