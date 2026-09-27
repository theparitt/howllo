"use client";

import { useEffect, useState } from "react";
import {
  managerIdentityMemberSessions,
  managerIdentityMembers,
  managerIdentityStatus,
  managerRemoveIdentityMember,
  managerRevokeIdentityMemberSessions,
} from "@/lib/api";
import type { IdentityDirectoryMember, IdentityDirectorySession, IdentityDirectoryPage } from "@/lib/types";
import { readStoredBearerToken } from "@/components/dev-auth-panel";

export function IdentityDirectory({ tenant, canRevoke }: { tenant: string; canRevoke: boolean }) {
  const [enabled, setEnabled] = useState(false);
  const [items, setItems] = useState<IdentityDirectoryPage | null>(null);
  const [page, setPage] = useState(1);
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState("");
  const [error, setError] = useState("");
  const [refreshEpoch, setRefreshEpoch] = useState(0);

  useEffect(() => {
    let active = true;
    managerIdentityStatus(tenant, readStoredBearerToken(tenant).trim())
      .then((status) => { if (active) setEnabled(status.enabled); })
      .catch((cause) => { if (active) setError(cause instanceof Error ? cause.message : "Could not check sign-in accounts."); });
    return () => { active = false; };
  }, [tenant]);

  useEffect(() => {
    if (!enabled) return;
    let active = true;
    const token = readStoredBearerToken(tenant).trim();
    managerIdentityMembers(tenant, token, page, search)
      .then((result) => { if (active) { setItems(result); setError(""); } })
      .catch((cause) => { if (active) setError(cause instanceof Error ? cause.message : "Could not load sign-in accounts."); });
    return () => { active = false; };
  }, [enabled, tenant, page, search, refreshEpoch]);

  if (!enabled && !error) return null;

  return <section className="panel">
    <h2 className="section-title">Sign-in accounts</h2>
    <p className="section-subtitle">Accounts in the connected identity workspace. Workspace roles and board restrictions are managed in Howllo.</p>
    {enabled ? <>
      <form className="manage-form" style={{ marginTop: "1rem" }} onSubmit={(event) => { event.preventDefault(); setPage(1); setSearch(query.trim()); }}>
        <input className="manage-input" aria-label="Search sign-in accounts" placeholder="Search name or email" value={query} onChange={(event) => setQuery(event.target.value)} maxLength={256} />
        <button className="ghost-button" type="submit">Search</button>
      </form>
      <div className="list-stack" style={{ marginTop: "1rem" }}>
        {items?.items.map((member) => <IdentityMemberRow key={member.id} tenant={tenant} member={member} canRevoke={canRevoke} onRemoved={() => setRefreshEpoch((value) => value + 1)} />)}
      </div>
      {items?.items.length === 0 ? <p className="section-subtitle">No matching accounts.</p> : null}
      {items && items.total > items.page_size ? <div className="manage-row__actions" style={{ marginTop: "1rem" }}>
        <button className="ghost-button" type="button" disabled={page <= 1} onClick={() => setPage((value) => value - 1)}>Previous</button>
        <span className="section-subtitle">Page {page} of {Math.ceil(items.total / items.page_size)}</span>
        <button className="ghost-button" type="button" disabled={page * items.page_size >= items.total} onClick={() => setPage((value) => value + 1)}>Next</button>
      </div> : null}
    </> : null}
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}

function IdentityMemberRow({ tenant, member, canRevoke, onRemoved }: { tenant: string; member: IdentityDirectoryMember; canRevoke: boolean; onRemoved: () => void }) {
  const [open, setOpen] = useState(false);
  const [sessions, setSessions] = useState<IdentityDirectorySession[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const showSessions = async () => {
    if (open) { setOpen(false); return; }
    setBusy(true); setError("");
    try {
      const rows = await managerIdentityMemberSessions(tenant, member.id, readStoredBearerToken(tenant).trim());
      setSessions(rows); setOpen(true);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not load sessions.");
    } finally { setBusy(false); }
  };

  const revoke = async () => {
    if (!window.confirm(`Sign out all sessions for ${member.display_name || member.email || "this account"}?`)) return;
    setBusy(true); setError("");
    try {
      await managerRevokeIdentityMemberSessions(tenant, member.id, readStoredBearerToken(tenant).trim());
      setSessions([]);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not revoke sessions.");
    } finally { setBusy(false); }
  };

  const remove = async () => {
    if (!window.confirm(`Remove ${member.display_name || member.email || "this account"} from this sign-in workspace and Howllo? Their sessions here will end.`)) return;
    setBusy(true); setError("");
    try {
      await managerRemoveIdentityMember(tenant, member.id, readStoredBearerToken(tenant).trim());
      onRemoved();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not remove account.");
    } finally { setBusy(false); }
  };

  return <div className="manage-row" style={{ display: "block" }}>
    <div className="manage-row__actions" style={{ justifyContent: "space-between" }}>
      <div><strong>{member.display_name || member.email || "Account"}</strong><p className="section-subtitle">{member.email || "No email"} · {member.status}</p></div>
      <div className="manage-row__actions">
        <button className="ghost-button" type="button" disabled={busy} onClick={() => void showSessions()}>{open ? "Hide sessions" : "Sessions"}</button>
        {canRevoke ? <button className="ghost-button" type="button" disabled={busy} onClick={() => void remove()}>Remove account</button> : null}
      </div>
    </div>
    {open ? <div style={{ marginTop: "0.75rem" }}>
      {sessions?.length ? sessions.map((session) => <p className="section-subtitle" key={session.id}>{session.user_agent || "Unknown device"} · {session.ip || "Unknown IP"} · {new Date(session.last_seen_at || session.created_at).toLocaleString()}</p>) : <p className="section-subtitle">No active sessions.</p>}
      {canRevoke && sessions?.length ? <button className="ghost-button" type="button" disabled={busy} onClick={() => void revoke()}>Sign out all sessions</button> : null}
    </div> : null}
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </div>;
}
