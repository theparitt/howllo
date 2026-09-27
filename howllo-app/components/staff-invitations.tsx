"use client";

import { useEffect, useState } from "react";
import { getMyInvitations, redeemInvitation, respondToInvitation } from "@/lib/api";
import { readAccountToken, subscribeToBearerTokenChange } from "@/components/dev-auth-panel";
import type { MyInvitation } from "@/lib/types";

export function StaffInvitations() {
  const [token, setToken] = useState("");
  const [items, setItems] = useState<MyInvitation[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [code, setCode] = useState("");

  useEffect(() => {
    const sync = () => setToken(readAccountToken().trim());
    sync();
    return subscribeToBearerTokenChange(sync);
  }, []);
  useEffect(() => {
    if (!token) { setItems([]); return; }
    let cancelled = false;
    getMyInvitations(token).then((invitations) => {
      if (!cancelled) setItems(invitations);
    }).catch(() => { if (!cancelled) setError("Could not load staff invitations."); });
    return () => { cancelled = true; };
  }, [token]);

  if (!token) return null;
  const redeem = async () => {
    setBusy("redeem");
    setError("");
    try {
      await redeemInvitation(code.trim(), token);
      setCode("");
      window.location.reload();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not redeem invitation.");
    } finally {
      setBusy(null);
    }
  };
  const respond = async (item: MyInvitation, action: "accept" | "reject") => {
    setBusy(item.id);
    setError("");
    try {
      await respondToInvitation(item.id, action, token);
      setItems((current) => current.filter((invite) => invite.id !== item.id));
      if (action === "accept") window.location.reload();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not respond to invitation.");
    } finally {
      setBusy(null);
    }
  };

  return <section className="panel" style={{ maxWidth: "48rem", margin: "1rem auto" }}>
    <h2 className="section-title">Staff invitations</h2>
    <p className="section-subtitle">Accept an invitation or enter a code from a workspace owner.</p>
    <form className="manage-form" onSubmit={(event) => { event.preventDefault(); void redeem(); }}>
      <input className="manage-input" aria-label="Invitation code" placeholder="Invitation code" value={code} onChange={(event) => setCode(event.target.value)} autoComplete="off" />
      <button className="button button--cta" type="submit" disabled={!code.trim() || Boolean(busy)}>Join workspace</button>
    </form>
    {items.map((item) => <div className="manage-row" key={item.id}>
      <div><strong>{item.tenant_name}</strong><p className="section-subtitle">{item.role} · {item.email}</p></div>
      <div className="manage-row__actions">
        <button className="button button--cta" disabled={Boolean(busy)} onClick={() => void respond(item, "accept")}>Accept</button>
        <button className="ghost-button" disabled={Boolean(busy)} onClick={() => void respond(item, "reject")}>Decline</button>
      </div>
    </div>)}
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}
