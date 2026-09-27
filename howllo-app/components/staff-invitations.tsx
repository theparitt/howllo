"use client";

import { useEffect, useState } from "react";
import { ApiError, createWorkspaceSession, declineInvitationCode, getMyInvitations, redeemInvitation, respondToInvitation } from "@/lib/api";
import { persistBearerToken, readAccountToken, subscribeToBearerTokenChange } from "@/components/dev-auth-panel";
import type { MyInvitation } from "@/lib/types";

const RESULT_KEY = "howllo.staff.invitation-result";

function invitationError(cause: unknown): string {
  if (cause instanceof ApiError) {
    if (cause.status === 404) return "This code is invalid, expired, withdrawn, or already used.";
    if (cause.status === 429) return "Too many attempts. Try again in a minute.";
    if (cause.status === 401) return "Your sign-in expired. Sign in again to continue.";
  }
  return cause instanceof Error ? cause.message : "Could not complete this invitation.";
}

export function StaffInvitations() {
  const [token, setToken] = useState("");
  const [items, setItems] = useState<MyInvitation[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [result, setResult] = useState("");
  const [code, setCode] = useState("");
  const [codeOpen, setCodeOpen] = useState(false);

  useEffect(() => {
    const sync = () => setToken(readAccountToken().trim());
    sync();
    const savedResult = window.sessionStorage.getItem(RESULT_KEY);
    if (savedResult) {
      setResult(savedResult);
      window.sessionStorage.removeItem(RESULT_KEY);
    }
    return subscribeToBearerTokenChange(sync);
  }, []);

  useEffect(() => {
    if (!token) { setItems([]); return; }
    let cancelled = false;
    setError("");
    getMyInvitations(token).then((invitations) => {
      if (!cancelled) setItems(invitations);
    }).catch(() => {
      if (!cancelled) setError("Could not load invitations. Refresh the page to try again.");
    });
    return () => { cancelled = true; };
  }, [token]);

  if (!token) return null;

  const redeem = async () => {
    if (!code.trim() || busy) return;
    setBusy("redeem");
    setError("");
    setResult("");
    try {
      await redeemInvitation(code.trim(), token);
      window.sessionStorage.setItem(RESULT_KEY, "Invitation accepted. Your workspace is listed below.");
      window.location.reload();
    } catch (cause) {
      setError(invitationError(cause));
      setBusy(null);
    }
  };

  const declineCode = async () => {
    if (!code.trim() || busy || !window.confirm("Decline this invitation? The code cannot be used again.")) return;
    setBusy("decline");
    setError("");
    setResult("");
    try {
      await declineInvitationCode(code.trim(), token);
      setCode("");
      setCodeOpen(false);
      setResult("Invitation declined. The workspace owner has been notified.");
    } catch (cause) {
      setError(invitationError(cause));
    } finally {
      setBusy(null);
    }
  };

  const respond = async (item: MyInvitation, action: "accept" | "reject") => {
    if (busy) return;
    setBusy(item.id);
    setError("");
    setResult("");
    try {
      await respondToInvitation(item.id, action, token);
      setItems((current) => current.filter((invite) => invite.id !== item.id));
      if (action === "reject") {
        setResult(`Invitation to ${item.tenant_name} declined.`);
        setBusy(null);
      } else {
        try {
          const session = await createWorkspaceSession({
            tenantSlug: item.tenant_slug,
            accessToken: token.replace(/^Bearer\s+/i, ""),
          });
          persistBearerToken(item.tenant_slug, `Bearer ${session.session_token}`);
          window.location.assign(`/app/${encodeURIComponent(item.tenant_slug)}`);
        } catch {
          window.sessionStorage.setItem(RESULT_KEY, "Invitation accepted. Open your workspace from the list below.");
          window.location.reload();
        }
      }
    } catch (cause) {
      setError(action === "accept" && item.provider_id && cause instanceof ApiError && (cause.status === 403 || cause.status === 404)
        ? "Accept the invitation in your sign-in provider first, then try again here."
        : invitationError(cause));
      setBusy(null);
    }
  };

  const expanded = items.length > 0 || codeOpen || Boolean(error) || Boolean(result);
  return <section className={expanded ? "panel staff-invitations" : "staff-invitations staff-invitations--compact"} aria-label="Staff invitations">
    {expanded ? <div className="staff-invitations__heading">
      <h2 className="section-title">Staff invitations</h2>
      <details className="staff-invitations__help">
        <summary aria-label="About staff invitations">?</summary>
        <p>An owner or admin invites you to help manage a workspace. If your invitation came through your sign-in provider, accept its email first. Then join here.</p>
      </details>
    </div> : null}
    {items.map((item) => <div className="manage-row" key={item.id}>
      <div>
        <strong>{item.tenant_name}</strong>
        <p className="section-subtitle">Invited as {item.role}{item.invited_by_name ? ` by ${item.invited_by_name}` : ""} · {item.email}</p>
      </div>
      <div className="manage-row__actions">
        <button className="button button--cta" type="button" disabled={Boolean(busy)} onClick={() => void respond(item, "accept")}>{busy === item.id ? "Opening…" : item.provider_id ? "Join workspace" : "Accept"}</button>
        <button className="ghost-button" type="button" disabled={Boolean(busy)} onClick={() => void respond(item, "reject")}>Decline</button>
      </div>
    </div>)}
    <button className="ghost-button staff-invitations__code-toggle" type="button" aria-expanded={codeOpen} aria-controls="staff-invitation-code" onClick={() => { if (codeOpen) setCode(""); setCodeOpen(!codeOpen); setError(""); }}>
      {codeOpen ? "Hide code entry" : "Have an invitation code?"}
    </button>
    {codeOpen ? <form id="staff-invitation-code" className="staff-invitations__code-form" onSubmit={(event) => { event.preventDefault(); void redeem(); }}>
      <label htmlFor="invitation-code">One-time invitation code</label>
      <div className="manage-form">
        <input id="invitation-code" className="manage-input" placeholder="howllo_inv_…" value={code} onChange={(event) => setCode(event.target.value)} autoComplete="off" spellCheck={false} required />
        <button className="button button--cta" type="submit" disabled={!code.trim() || Boolean(busy)}>{busy === "redeem" ? "Joining…" : "Join workspace"}</button>
        <button className="ghost-button" type="button" disabled={!code.trim() || Boolean(busy)} onClick={() => void declineCode()}>{busy === "decline" ? "Declining…" : "Decline invitation"}</button>
      </div>
      <p className="section-subtitle">Paste the code sent privately by your workspace owner. Each code can be used once.</p>
    </form> : null}
    {result ? <p className="notice" role="status">{result}</p> : null}
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}
