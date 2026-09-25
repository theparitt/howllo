"use client";

import { useCallback, useEffect, useState } from "react";
import { managerModerationQueue, moderateReview, moderateVisibility } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import type { ModerationQueueItem } from "@/lib/types";

export function ModerationTab({ tenant }: { tenant: string }) {
  const [items, setItems] = useState<ModerationQueueItem[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const token = () => readStoredBearerToken(tenant).trim();
  const reload = useCallback(async () => {
    try { setItems(await managerModerationQueue(tenant, readStoredBearerToken(tenant).trim())); setError(""); }
    catch (cause) { setError(cause instanceof Error ? cause.message : "Could not load moderation queue."); }
  }, [tenant]);
  useEffect(() => { void reload(); }, [reload]);
  const change = async (id: string, action: "approve" | "reject" | "show") => {
    setBusy(id); setError("");
    try {
      if (action === "show") await moderateVisibility(id, false, token());
      else await moderateReview(id, action, token());
      await reload();
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Moderation action failed."); }
    finally { setBusy(null); }
  };
  return <section className="panel">
    <h2 className="section-title">Moderation queue</h2>
    <p className="section-subtitle">Posts waiting for approval or hidden across this workspace.</p>
    {items.length === 0 ? <p className="section-subtitle">No posts need review.</p> : null}
    <div className="list-stack" style={{ marginTop: "1rem" }}>{items.map((item) => <div className="manage-row" key={item.id}>
      <div><strong>{item.title}</strong><p className="section-subtitle">{item.board_slug} · {item.author_display_name} · {item.review_state === "pending" ? "Waiting for approval" : "Hidden"}{item.review_reason ? ` · ${item.review_reason}` : ""}</p><p>{item.body}</p></div>
      <div className="manage-row__actions">
        {item.review_state === "pending" ? <>
          <button className="ghost-button" disabled={Boolean(busy)} onClick={() => void change(item.id, "approve")}>Approve</button>
          <button className="ghost-button" disabled={Boolean(busy)} onClick={() => void change(item.id, "reject")}>Reject</button>
        </> : null}
        {item.review_state === "approved" && item.is_hidden ? <button className="ghost-button" disabled={Boolean(busy)} onClick={() => void change(item.id, "show")}>Show</button> : null}
      </div>
    </div>)}</div>
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}
