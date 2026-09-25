"use client";

import { useCallback, useEffect, useState } from "react";
import { managerModerationQueue, moderateStatus, moderateVisibility } from "@/lib/api";
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
  const change = async (id: string, action: "planned" | "declined" | "hide" | "show") => {
    setBusy(id); setError("");
    try {
      if (action === "hide" || action === "show") await moderateVisibility(id, action === "hide", token());
      else await moderateStatus(id, action, "Reviewed by workspace staff", token());
      await reload();
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Moderation action failed."); }
    finally { setBusy(null); }
  };
  return <section className="panel">
    <h2 className="section-title">Moderation queue</h2>
    <p className="section-subtitle">Posts under review or hidden across this workspace.</p>
    {items.length === 0 ? <p className="section-subtitle">No posts need review.</p> : null}
    <div className="list-stack" style={{ marginTop: "1rem" }}>{items.map((item) => <div className="manage-row" key={item.id}>
      <div><strong>{item.title}</strong><p className="section-subtitle">{item.board_slug} · {item.author_display_name} · {item.status}{item.is_hidden ? " · hidden" : ""}</p></div>
      <div className="manage-row__actions">
        {item.status === "under_review" ? <>
          <button className="ghost-button" disabled={Boolean(busy)} onClick={() => void change(item.id, "planned")}>Plan</button>
          <button className="ghost-button" disabled={Boolean(busy)} onClick={() => void change(item.id, "declined")}>Decline</button>
        </> : null}
        <button className="ghost-button" disabled={Boolean(busy)} onClick={() => void change(item.id, item.is_hidden ? "show" : "hide")}>{item.is_hidden ? "Show" : "Hide"}</button>
      </div>
    </div>)}</div>
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}
