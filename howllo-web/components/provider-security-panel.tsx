"use client";

import { useCallback, useEffect, useState } from "react";
import { clearStoredBearerToken, readStoredBearerToken } from "@/components/dev-auth-panel";
import { providerAccountRequest } from "@/lib/auth-api";
import { ProviderMark } from "@/components/provider-mark";

type Session = { id: string; user_agent: string | null; ip: string | null; created_at: string; last_seen_at: string | null; is_current: boolean };
type Passkey = { id: string; name: string; created_at: string; last_used_at: string | null };
type Mfa = { totp_enabled: boolean; backup_codes_remaining: number };
type Capabilities = { passkey_allowed?: boolean; can_remove_passkey?: boolean; can_enable_totp?: boolean; can_disable_totp?: boolean; mfa_required?: boolean };
type LinkedAccounts = { primary_email: string | null; providers: { provider: string; linked: boolean; linked_email: string | null }[] };

export function ProviderSecurityPanel({ tenantSlug }: { tenantSlug: string }) {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [passkeys, setPasskeys] = useState<Passkey[]>([]);
  const [mfa, setMfa] = useState<Mfa | null>(null);
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null);
  const [linkedAccounts, setLinkedAccounts] = useState<LinkedAccounts | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [enrollment, setEnrollment] = useState<{ challenge_id: string; secret: string; otpauth_uri: string } | null>(null);
  const [code, setCode] = useState("");
  const [recoveryCodes, setRecoveryCodes] = useState<string[]>([]);

  const load = useCallback(async () => {
    const token = readStoredBearerToken(tenantSlug);
    if (!token) return;
    try {
      const [nextSessions, nextPasskeys, nextMfa, nextCapabilities, nextLinkedAccounts] = await Promise.all([
        providerAccountRequest<Session[]>(token, "sessions"),
        providerAccountRequest<Passkey[]>(token, "security/passkeys"),
        providerAccountRequest<Mfa>(token, "security/mfa"),
        providerAccountRequest<Capabilities>(token, "security/capabilities"),
        providerAccountRequest<LinkedAccounts>(token, "security/linked-accounts"),
      ]);
      setSessions(nextSessions);
      setPasskeys(nextPasskeys);
      setMfa(nextMfa);
      setCapabilities(nextCapabilities);
      setLinkedAccounts(nextLinkedAccounts);
      setError("");
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not load account security."); }
    finally { setLoading(false); }
  }, [tenantSlug]);

  useEffect(() => { void load(); }, [load]);

  async function action(path: string, method: string, body?: object) {
    const token = readStoredBearerToken(tenantSlug);
    if (!token) throw new Error("Sign in again to manage your account.");
    setBusy(true); setError(""); setMessage("");
    try {
      const result = await providerAccountRequest<Record<string, unknown>>(token, path, method, body);
      await load();
      return result;
    } catch (cause) {
      const text = cause instanceof Error ? cause.message : "Could not update account security.";
      setError(text);
      throw cause;
    } finally { setBusy(false); }
  }

  if (loading) return <article className="panel"><h2 className="section-title">Account security</h2><p>Loading…</p></article>;
  if (!capabilities) return <article className="panel"><h2 className="section-title">Account security</h2><p className="section-subtitle">{error}</p></article>;

  return <div className="grid">
    {linkedAccounts ? <article className="panel">
      <h2 className="section-title">Sign-in account</h2>
      <p className="section-subtitle">{linkedAccounts.primary_email || "No email on this account"}</p>
      <div className="linked-provider-list" aria-label="Connected sign-in providers">
        {linkedAccounts.providers.filter(provider => provider.linked).length ? linkedAccounts.providers.filter(provider => provider.linked).map(provider => <span className="linked-provider" key={provider.provider}><ProviderMark provider={provider.provider} />{provider.provider === "oidc" ? "OpenID Connect" : provider.provider.charAt(0).toUpperCase() + provider.provider.slice(1)}</span>) : <span className="section-subtitle">No social accounts connected</span>}
      </div>
      <a className="button button--secondary" style={{ marginTop: "1rem" }} href="https://app.rooiam.com/my/account" target="_blank" rel="noreferrer">Manage email and connected accounts ↗</a>
    </article> : null}
    <article className="panel">
      <h2 className="section-title">Sessions</h2>
      <p className="section-subtitle">Devices signed in to your account.</p>
      <div className="stack" style={{ marginTop: "1rem" }}>
        {sessions.length ? sessions.map(session => <div className="toolbar" key={session.id} style={{ justifyContent: "space-between" }}>
          <div><strong>{session.is_current ? "This device" : session.user_agent || "Unknown device"}</strong><div className="section-subtitle">{session.ip || "IP unavailable"} · Last active {new Date(session.last_seen_at || session.created_at).toLocaleString()}</div></div>
          <button className="button button--secondary" disabled={busy} onClick={() => void action(`sessions/${session.id}`, "DELETE").then(() => {
            if (session.is_current) { clearStoredBearerToken(tenantSlug); window.location.assign("/"); }
            else setMessage("Device signed out.");
          }).catch(() => {})}>Sign out</button>
        </div>) : <p>No active sessions.</p>}
      </div>
      {sessions.length > 1 ? <button className="button button--secondary" style={{ marginTop: "1rem" }} disabled={busy} onClick={() => void action("sessions/revoke-others", "POST").then(() => setMessage("Other devices signed out.")).catch(() => {})}>Sign out other devices</button> : null}
    </article>

    <article className="panel">
      <h2 className="section-title">Passkeys</h2>
      <p className="section-subtitle">Use your device unlock to sign in.</p>
      {passkeys.map(passkey => <div className="toolbar" key={passkey.id} style={{ justifyContent: "space-between", marginTop: "0.9rem" }}>
        <span>{passkey.name} <small>· Added {new Date(passkey.created_at).toLocaleDateString()}</small></span>
        <span className="toolbar">
          <button className="button button--secondary" disabled={busy} onClick={() => {
            const name = window.prompt("Passkey name", passkey.name)?.trim();
            if (name && name !== passkey.name) void action(`security/passkeys/${passkey.id}`, "PATCH", { name }).then(() => setMessage("Passkey renamed.")).catch(() => {});
          }}>Rename</button>
          <button className="button button--secondary" disabled={busy || (passkeys.length === 1 && !capabilities.can_remove_passkey)} onClick={() => {
            if (window.confirm(`Remove ${passkey.name}?`)) void action(`security/passkeys/${passkey.id}`, "DELETE").then(() => setMessage("Passkey removed.")).catch(() => {});
          }}>Remove</button>
        </span>
      </div>)}
      {capabilities.passkey_allowed ? <p style={{ marginTop: "1rem" }}><a className="button button--secondary" href="https://app.rooiam.com/my/security" target="_blank" rel="noreferrer">Add passkey ↗</a></p> : null}
    </article>

    <article className="panel">
      <h2 className="section-title">Two-step verification</h2>
      <p className="section-subtitle">{mfa?.totp_enabled ? `Authenticator enabled · ${mfa.backup_codes_remaining} recovery codes left` : "Add an authenticator app for extra security."}</p>
      {!mfa?.totp_enabled && capabilities.can_enable_totp ? <button className="button" disabled={busy} style={{ marginTop: "1rem" }} onClick={() => void action("security/mfa/totp/start", "POST").then(result => setEnrollment(result as typeof enrollment & { challenge_id: string; secret: string; otpauth_uri: string })).catch(() => {})}>Set up authenticator</button> : null}
      {enrollment ? <div className="field-grid" style={{ marginTop: "1rem" }}>
        <p>Enter this key in your authenticator app, then enter its six-digit code.</p>
        <code style={{ overflowWrap: "anywhere", userSelect: "all" }}>{enrollment.secret}</code>
        <label className="field-label">Verification code<input className="field" inputMode="numeric" maxLength={6} value={code} onChange={event => setCode(event.target.value.replace(/\D/g, ""))} /></label>
        <button className="button" disabled={busy || code.length !== 6} onClick={() => void action("security/mfa/totp/finish", "POST", { challenge_id: enrollment.challenge_id, code }).then(result => { setRecoveryCodes(result.backup_codes as string[]); setEnrollment(null); setCode(""); }).catch(() => {})}>Enable two-step verification</button>
      </div> : null}
      {mfa?.totp_enabled ? <div className="toolbar" style={{ marginTop: "1rem" }}>
        <button className="button button--secondary" disabled={busy} onClick={() => { if (window.confirm("Replace your recovery codes? Old codes will stop working.")) void action("security/mfa/recovery-codes", "POST").then(result => setRecoveryCodes(result.codes as string[])).catch(() => {}); }}>New recovery codes</button>
        {capabilities.can_disable_totp ? <button className="button button--secondary" disabled={busy} onClick={() => { if (window.confirm("Turn off two-step verification? Other devices will be signed out.")) void action("security/mfa/totp", "DELETE").then(() => setMessage("Two-step verification disabled.")).catch(() => {}); }}>Turn off</button> : null}
      </div> : null}
      {recoveryCodes.length ? <div className="notice" role="status" style={{ marginTop: "1rem" }}><strong>Save these codes now. They will not be shown again.</strong><pre style={{ whiteSpace: "pre-wrap", userSelect: "all" }}>{recoveryCodes.join("\n")}</pre><button className="button button--secondary" onClick={() => setRecoveryCodes([])}>I saved them</button></div> : null}
    </article>
    {message ? <div className="notice" role="status">{message}</div> : null}
    {error ? <div className="notice notice--error" role="alert">{error}</div> : null}
  </div>;
}
