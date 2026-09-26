"use client";

import { useCallback, useEffect, useState } from "react";
import { getBoardPosts, moderatePin } from "@/lib/api";
import type { PostListItem } from "@/lib/types";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import { LIST_PAGE_SIZE, ListPager, ListSearch } from "./list-controls";

export function BoardTopics({ tenant, boardSlug }: { tenant: string; boardSlug: string }) {
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(1);
  const [topics, setTopics] = useState<PostListItem[]>([]);
  const [total, setTotal] = useState(0);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const token = () => readStoredBearerToken(tenant).trim();

  const load = useCallback(async () => {
    const result = await getBoardPosts({ tenantSlug: tenant, boardSlug, q: search.trim() || undefined, page, perPage: LIST_PAGE_SIZE, token: token() });
    setTopics(result.items); setTotal(result.total);
  }, [tenant, boardSlug, search, page]);

  useEffect(() => {
    let active = true;
    getBoardPosts({ tenantSlug: tenant, boardSlug, q: search.trim() || undefined, page, perPage: LIST_PAGE_SIZE, token: token() })
      .then((result) => { if (active) { setTopics(result.items); setTotal(result.total); } })
      .catch((cause) => { if (active) setError(cause instanceof Error ? cause.message : "Could not load topics."); });
    return () => { active = false; };
  }, [tenant, boardSlug, search, page]);

  async function toggle(post: PostListItem) {
    setBusy(post.id); setError("");
    try { await moderatePin(post.id, !post.is_pinned, token()); await load(); }
    catch (cause) { setError(cause instanceof Error ? cause.message : "Could not update topic."); }
    finally { setBusy(null); }
  }

  return <details className="board-extras">
    <summary>Topics and pinned posts</summary>
    <div style={{ marginTop: "1rem" }}><ListSearch value={search} onChange={(value) => { setSearch(value); setPage(1); }} placeholder="Search topics" /></div>
    {topics.length ? topics.map((post) => <div className="manage-row" key={post.id} style={{ justifyContent: "space-between", gap: "1rem" }}>
      <div><strong>{post.title}</strong><p className="section-subtitle">{post.is_pinned ? "Pinned · " : ""}{post.comment_count} comments · {post.vote_count} votes</p></div>
      <button type="button" className="ghost-button" disabled={busy !== null} onClick={() => void toggle(post)}>{post.is_pinned ? "Unpin" : "Pin"}</button>
    </div>) : <p className="section-subtitle">No topics yet.</p>}
    <ListPager page={page} total={total} onPage={setPage} />
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </details>;
}
