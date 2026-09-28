"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { managerClearParticipantRestriction, managerListParticipants, managerSetParticipantRestriction } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import type { WorkspaceParticipant } from "@/lib/types";
import { LIST_PAGE_SIZE, ListPager, ListSearch } from "./list-controls";

export function ParticipantsTab({ tenant }: { tenant: string }) {
  const [items, setItems] = useState<WorkspaceParticipant[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [page, setPage] = useState(1);
  const refresh = useCallback(async () => {
    try {
      setItems(await managerListParticipants(tenant, readStoredBearerToken(tenant).trim()));
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not load workspace users.");
    } finally { setLoading(false); }
  }, [tenant]);
  useEffect(() => { void refresh(); }, [refresh]);
  const visible = useMemo(() => items.filter((item) => {
    const matchesQuery = `${item.display_name} ${item.email}`.toLowerCase().includes(query.trim().toLowerCase());
    const matchesFilter = filter === "all" || (filter === "active" ? !item.restriction_kind : item.restriction_kind === filter);
    return matchesQuery && matchesFilter;
  }), [items, query, filter]);
  const safePage = Math.min(page, Math.max(1, Math.ceil(visible.length / LIST_PAGE_SIZE)));

  return <section className="panel">
    <h2 className="section-title">People on your boards</h2>
    <p className="section-subtitle">People who joined this workspace through the public board. Restrictions apply to every board in this workspace.</p>
    <ListSearch value={query} onChange={(value) => { setQuery(value); setPage(1); }} placeholder="Search users by name or email"><select className="manage-input" aria-label="Filter users by status" value={filter} onChange={(event) => { setFilter(event.target.value); setPage(1); }}><option value="all">All statuses</option><option value="active">Active</option><option value="suspended">Suspended</option><option value="banned">Banned</option></select></ListSearch>
    {loading ? <p>Loading…</p> : null}
    {items.length === 0 && !loading ? <p className="section-subtitle">No public users have joined yet.</p> : null}
    <div className="list-stack" style={{ marginTop: "1rem" }}>
      {visible.slice((safePage - 1) * LIST_PAGE_SIZE, safePage * LIST_PAGE_SIZE).map((item) => <ParticipantRow key={item.user_id} item={item} tenant={tenant} onChanged={refresh} />)}
      {items.length > 0 && visible.length === 0 ? <p className="section-subtitle">No matching users.</p> : null}
    </div>
    <ListPager page={safePage} total={visible.length} onPage={setPage} />
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}

function ParticipantRow({ item, tenant, onChanged }: { item: WorkspaceParticipant; tenant: string; onChanged: () => Promise<void> }) {
  const [mode, setMode] = useState<"suspended" | "banned">("suspended");
  const [days, setDays] = useState(7);
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [editing, setEditing] = useState(false);
  const manageButton = useRef<HTMLButtonElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const restriction = item.restriction_kind;
  useEffect(() => {
    if (!editing) return;
    closeButton.current?.focus();
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") { setEditing(false); manageButton.current?.focus(); }
      if (event.key === "Tab") {
        const dialog = document.querySelector<HTMLElement>(`[data-member-dialog="${item.user_id}"]`);
        const focusables = Array.from(dialog?.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), select:not([disabled])') ?? []);
        if (event.shiftKey && document.activeElement === focusables[0]) { event.preventDefault(); focusables.at(-1)?.focus(); }
        else if (!event.shiftKey && document.activeElement === focusables.at(-1)) { event.preventDefault(); focusables[0]?.focus(); }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => { document.body.style.overflow = previousOverflow; document.removeEventListener("keydown", onKey); };
  }, [editing, item.user_id]);
  const apply = async () => {
    if (!reason.trim()) { setError("Enter a reason."); return; }
    if (!window.confirm(`${mode === "banned" ? "Ban" : "Suspend"} ${item.display_name} across every board in this workspace?`)) return;
    setBusy(true); setError("");
    try {
      await managerSetParticipantRestriction(tenant, item.user_id, readStoredBearerToken(tenant).trim(), {
        kind: mode, duration_days: mode === "suspended" ? days : undefined, reason: reason.trim(),
      });
      setReason("");
      await onChanged();
      setEditing(false);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not restrict user."); }
    finally { setBusy(false); }
  };
  const lift = async () => {
    if (!window.confirm(`Lift ${item.display_name}'s restriction across every board in this workspace?`)) return;
    setBusy(true); setError("");
    try {
      await managerClearParticipantRestriction(tenant, item.user_id, readStoredBearerToken(tenant).trim());
      await onChanged();
      setEditing(false);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not lift restriction."); }
    finally { setBusy(false); }
  };

  return <div className="manage-row member-row">
    <div className="member-row__person">
      <span className="member-row__avatar" aria-hidden="true">{(item.display_name || item.email || "?").slice(0, 1).toUpperCase()}</span><div>
      <strong>{item.display_name}</strong>
      <p className="section-subtitle">{item.email || `User ${item.user_id.slice(0, 8)}`}</p>
      </div></div>
    <span className="member-row__status" data-status={restriction ?? "active"}>{restriction ? restriction === "banned" ? "Banned" : "Suspended" : "Active"}</span>
    <button ref={manageButton} type="button" className="ghost-button button--small" aria-haspopup="dialog" aria-expanded={editing} onClick={() => setEditing(true)}>Manage</button>
    {editing ? <div className="member-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) { setEditing(false); manageButton.current?.focus(); } }}><div className="member-dialog panel" data-member-dialog={item.user_id} role="dialog" aria-modal="true" aria-label={`Manage ${item.display_name}`}>
      <div className="member-dialog__head"><div><h2 className="section-title">{item.display_name}</h2><p className="section-subtitle">Restrictions apply to every board in this workspace.</p></div><button ref={closeButton} type="button" className="ghost-button" onClick={() => { setEditing(false); manageButton.current?.focus(); }} aria-label="Close member settings">×</button></div>
      {restriction ? <p className="member-dialog__current">Currently {restriction}{item.restriction_expires_at ? ` until ${new Date(item.restriction_expires_at).toLocaleDateString()}` : ""}{item.restriction_reason ? ` · ${item.restriction_reason}` : ""}</p> : null}
      <div className="manage-fields member-row__controls">
      <select className="manage-input" aria-label="Restriction type" value={mode} disabled={busy} onChange={(event) => setMode(event.target.value as "suspended" | "banned")}>
        <option value="suspended">Temporary suspension</option><option value="banned">Ban until lifted</option>
      </select>
      {mode === "suspended" ? <select className="manage-input" aria-label="Suspension length" value={days} disabled={busy} onChange={(event) => setDays(Number(event.target.value))}>
        <option value={1}>1 day</option><option value={7}>7 days</option><option value={30}>30 days</option><option value={90}>90 days</option>
      </select> : null}
      <input className="manage-input" aria-label="Restriction reason" placeholder="Reason (required)" maxLength={500} value={reason} disabled={busy} onChange={(event) => setReason(event.target.value)} />
      <div className="manage-row__actions">
        <button className="button button--cta" type="button" disabled={busy || !reason.trim()} onClick={() => void apply()}>{busy ? "Saving…" : mode === "banned" ? "Ban user" : `Suspend ${days} ${days === 1 ? "day" : "days"}`}</button>
        {restriction ? <button className="ghost-button" type="button" disabled={busy} onClick={() => void lift()}>Lift restriction</button> : null}
      </div>
      {error ? <p className="error-text" role="alert">{error}</p> : null}
    </div></div></div> : null}
  </div>;
}
