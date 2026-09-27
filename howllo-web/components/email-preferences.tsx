"use client";

import { useEffect, useState } from "react";
import { API_BASE_URL } from "@/lib/config";
import { readAccountToken, readStoredBearerToken } from "@/components/dev-auth-panel";

type Preferences = {
  verified_email: string | null; replies_enabled: boolean; updates_enabled: boolean;
  digest_enabled: boolean; broadcast_enabled: boolean;
};

export function EmailPreferences({ tenantSlug }: { tenantSlug: string }) {
  const [available, setAvailable] = useState(false);
  const [prefs, setPrefs] = useState<Preferences | null>(null);
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const endpoint = `${API_BASE_URL}/api/me/email-preferences?tenant_slug=${encodeURIComponent(tenantSlug)}`;
  async function request(url: string, method = "GET", body?: object, account = false) {
    const response = await fetch(url, { method, cache: "no-store", headers: { Authorization: account ? (readAccountToken().trim() || readStoredBearerToken(tenantSlug).trim()) : readStoredBearerToken(tenantSlug).trim(), "content-type": "application/json" }, body: body ? JSON.stringify(body) : undefined });
    if (!response.ok) throw new Error(response.status === 429 ? "Too many requests. Please try later." : (await response.text()) || "Could not update email settings.");
    return response.status === 202 ? null : response.json();
  }
  useEffect(() => {
    let active = true;
    fetch(`${API_BASE_URL}/api/email/availability`, { cache: "no-store" }).then((r) => r.json())
      .then((result: { enabled: boolean }) => { if (active) setAvailable(result.enabled); if (result.enabled) return request(endpoint).then((p: Preferences) => { if (active) setPrefs(p); }).catch(() => {}); })
      .catch(() => {});
    return () => { active = false; };
  }, [tenantSlug]);
  async function action(work: () => Promise<unknown>, success: string) {
    setBusy(true); setError(""); setMessage("");
    try { await work(); setMessage(success); try { setPrefs(await request(endpoint) as Preferences); } catch {} }
    catch (cause) { setError(cause instanceof Error ? cause.message : "Email request failed."); }
    finally { setBusy(false); }
  }
  if (!available) return null;
  return <article className="panel">
    <h2 className="section-title">Email</h2>
    {prefs?.verified_email ? <p className="section-subtitle">Verified: {prefs.verified_email}</p> : <p className="section-subtitle">Verify an email address to receive updates and use email password recovery.</p>}
    <form className="field-grid" onSubmit={(event) => { event.preventDefault(); void action(() => request(`${API_BASE_URL}/api/me/email-verification/request`, "POST", { email }, true), "Verification code sent. Check your inbox."); }}>
      <label className="field-label">Email address<input className="field" type="email" required value={email} onChange={(e) => setEmail(e.target.value)} placeholder={prefs?.verified_email ?? "you@example.com"} /></label>
      <button className="button" disabled={busy}>Send verification code</button>
    </form>
    <form className="field-grid" style={{ marginTop: "1rem" }} onSubmit={(event) => { event.preventDefault(); void action(() => request(`${API_BASE_URL}/api/me/email-verification/confirm`, "POST", { token: code }, true), "Email verified."); }}>
      <label className="field-label">Verification code<input className="field" value={code} onChange={(e) => setCode(e.target.value)} autoComplete="one-time-code" required /></label>
      <button className="button" disabled={busy || !code}>Verify email</button>
    </form>
    {prefs ? <form className="field-grid" style={{ marginTop: "1.5rem" }} onSubmit={(event) => { event.preventDefault(); void action(() => request(endpoint, "PUT", prefs), "Email preferences saved."); }}>
      <h3 className="section-title">Updates from this workspace</h3>
      {([ ["replies_enabled", "Replies to posts"], ["updates_enabled", "Important post updates"], ["digest_enabled", "Daily digest"], ["broadcast_enabled", "Announcements"] ] as const).map(([key, label]) => <label key={key}><input type="checkbox" checked={prefs[key]} onChange={(e) => setPrefs({ ...prefs, [key]: e.target.checked })} /> {label}</label>)}
      <button className="button" disabled={busy}>Save preferences</button>
    </form> : null}
    {message ? <p className="notice" role="status">{message}</p> : null}
    {error ? <p className="notice notice--error" role="alert">{error}</p> : null}
  </article>;
}
