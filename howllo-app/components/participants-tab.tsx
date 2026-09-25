"use client";

import { useCallback, useEffect, useState } from "react";
import { managerClearParticipantRestriction, managerListParticipants, managerSetParticipantRestriction } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import type { WorkspaceParticipant } from "@/lib/types";

export function ParticipantsTab({ tenant }: { tenant: string }) {
  const [items, setItems] = useState<WorkspaceParticipant[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const refresh = useCallback(async () => {
    try {
      setItems(await managerListParticipants(tenant, readStoredBearerToken(tenant).trim()));
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not load workspace users.");
    } finally { setLoading(false); }
  }, [tenant]);
  useEffect(() => { void refresh(); }, [refresh]);

  return <section className="panel">
    <h2 className="section-title">Workspace users</h2>
    <p className="section-subtitle">People who joined this workspace through the public board. Restrictions apply to every board in this workspace.</p>
    {loading ? <p>Loading…</p> : null}
    {items.length === 0 && !loading ? <p className="section-subtitle">No public users have joined yet.</p> : null}
    <div className="list-stack" style={{ marginTop: "1rem" }}>
      {items.map((item) => <ParticipantRow key={item.user_id} item={item} tenant={tenant} onChanged={refresh} />)}
    </div>
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}

function ParticipantRow({ item, tenant, onChanged }: { item: WorkspaceParticipant; tenant: string; onChanged: () => Promise<void> }) {
  const [mode, setMode] = useState<"suspended" | "banned">("suspended");
  const [days, setDays] = useState(7);
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const restriction = item.restriction_kind;
  const apply = async () => {
    if (!reason.trim()) { setError("Enter a reason."); return; }
    setBusy(true); setError("");
    try {
      await managerSetParticipantRestriction(tenant, item.user_id, readStoredBearerToken(tenant).trim(), {
        kind: mode, duration_days: mode === "suspended" ? days : undefined, reason: reason.trim(),
      });
      setReason("");
      await onChanged();
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not restrict user."); }
    finally { setBusy(false); }
  };
  const lift = async () => {
    setBusy(true); setError("");
    try {
      await managerClearParticipantRestriction(tenant, item.user_id, readStoredBearerToken(tenant).trim());
      await onChanged();
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not lift restriction."); }
    finally { setBusy(false); }
  };

  return <div className="manage-row" style={{ alignItems: "flex-start" }}>
    <div style={{ flex: 1 }}>
      <strong>{item.display_name}</strong>
      <p className="section-subtitle">{item.email || `User ${item.user_id.slice(0, 8)}`}</p>
      {restriction ? <p className="section-subtitle"><strong>{restriction === "banned" ? "Banned" : "Suspended"}</strong>
        {item.restriction_expires_at ? ` until ${new Date(item.restriction_expires_at).toLocaleDateString()}` : ""}
        {item.restriction_reason ? ` · ${item.restriction_reason}` : ""}
      </p> : <p className="section-subtitle">Active</p>}
      {error ? <p className="error-text" role="alert">{error}</p> : null}
    </div>
    <div className="manage-fields" style={{ width: "min(20rem, 100%)" }}>
      <select className="manage-input" aria-label="Restriction type" value={mode} disabled={busy} onChange={(event) => setMode(event.target.value as "suspended" | "banned")}>
        <option value="suspended">Temporary suspension</option><option value="banned">Ban until lifted</option>
      </select>
      {mode === "suspended" ? <select className="manage-input" aria-label="Suspension length" value={days} disabled={busy} onChange={(event) => setDays(Number(event.target.value))}>
        <option value={1}>1 day</option><option value={7}>7 days</option><option value={30}>30 days</option><option value={90}>90 days</option>
      </select> : null}
      <input className="manage-input" aria-label="Restriction reason" placeholder="Reason (required)" maxLength={500} value={reason} disabled={busy} onChange={(event) => setReason(event.target.value)} />
      <div className="manage-row__actions">
        <button className="button button--cta" type="button" disabled={busy || !reason.trim()} onClick={() => void apply()}>{busy ? "Saving…" : "Apply"}</button>
        {restriction ? <button className="ghost-button" type="button" disabled={busy} onClick={() => void lift()}>Lift restriction</button> : null}
      </div>
    </div>
  </div>;
}
