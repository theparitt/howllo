import { useEffect, useState } from "react";
import { useSession } from "../../lib/session";
import { Panel } from "../../components/Panel";

type EmailConfig = {
  smtp_host: string; smtp_port: number; smtp_tls_mode: "implicit" | "starttls" | "none";
  smtp_username: string; smtp_password_configured: boolean; from_email: string;
  from_name: string; reply_to: string; status: string; tested_at: string | null;
  failure_count: number; monthly_limit: number; tenant_daily_limit: number;
  tenant_monthly_limit: number; max_broadcast_recipients: number;
  broadcasts_per_day: number; broadcasts_per_week: number; monthly_used: number; config_key_available: boolean;
};
type SuppressedEmail = { recipient: string; reason: string; created_at: string };

export function PlatformEmailTab() {
  const { client } = useSession();
  const [config, setConfig] = useState<EmailConfig | null>(null);
  const [password, setPassword] = useState("");
  const [recipient, setRecipient] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [suppressed, setSuppressed] = useState<SuppressedEmail[]>([]);
  const [blockedAddress, setBlockedAddress] = useState("");
  const [blockedReason, setBlockedReason] = useState("");
  useEffect(() => {
    client.request<EmailConfig>("/api/admin/platform/email", { withTenant: false })
      .then(setConfig).catch((cause) => setError(cause instanceof Error ? cause.message : "Could not load email settings."));
    client.request<SuppressedEmail[]>("/api/admin/platform/email/suppression", { withTenant: false })
      .then(setSuppressed).catch(() => {});
  }, [client]);

  async function run(action: () => Promise<EmailConfig>, message: string) {
    setBusy(true); setError(""); setNotice("");
    try { setConfig(await action()); setPassword(""); setNotice(message); }
    catch (cause) { setError(cause instanceof Error ? cause.message : "Email action failed."); }
    finally { setBusy(false); }
  }
  if (!config) return <Panel title="Platform email"><p>{error || "Loading…"}</p></Panel>;
  const set = <K extends keyof EmailConfig>(key: K, value: EmailConfig[K]) => setConfig({ ...config, [key]: value });
  return <Panel title="Platform email">
    <p className="muted">Delivery is off until you save SMTP settings, send a test email, and enable it. Password recovery codes and in-app notifications still work when email is off.</p>
    <p role="status"><strong>Status: {config.status}</strong>{config.status === "paused" ? " · Sending paused after repeated SMTP failures. Check the server and send another test." : null}</p>
    <p className="muted">Monthly usage: {config.monthly_used} / {config.monthly_limit}{config.monthly_used >= config.monthly_limit * 0.9 ? " · Near the monthly limit" : config.monthly_used >= config.monthly_limit * 0.7 ? " · Usage above 70%" : ""}</p>
    {!config.config_key_available ? <p className="notice notice--error">Set HOWLLO_EMAIL_CONFIG_KEY on the server before saving settings.</p> : null}
    <form onSubmit={(event) => { event.preventDefault(); void run(() => client.request<EmailConfig>("/api/admin/platform/email", { method: "PUT", withTenant: false, body: JSON.stringify({
      smtp_host: config.smtp_host, smtp_port: config.smtp_port, smtp_tls_mode: config.smtp_tls_mode,
      smtp_username: config.smtp_username, smtp_password: password || undefined,
      from_email: config.from_email, from_name: config.from_name, reply_to: config.reply_to,
      monthly_limit: config.monthly_limit, tenant_daily_limit: config.tenant_daily_limit,
      tenant_monthly_limit: config.tenant_monthly_limit, max_broadcast_recipients: config.max_broadcast_recipients,
      broadcasts_per_day: config.broadcasts_per_day, broadcasts_per_week: config.broadcasts_per_week,
    }) }), "Settings saved. Send a test email before enabling delivery."); }}>
      <div className="field-grid">
        <label>SMTP host<input required value={config.smtp_host} onChange={(e) => set("smtp_host", e.target.value)} placeholder="smtp.example.com" /></label>
        <label>Port<input required type="number" min="1" max="65535" value={config.smtp_port} onChange={(e) => set("smtp_port", Number(e.target.value))} /></label>
        <label>Security<select value={config.smtp_tls_mode} onChange={(e) => set("smtp_tls_mode", e.target.value as EmailConfig["smtp_tls_mode"])}><option value="starttls">STARTTLS</option><option value="implicit">TLS</option><option value="none">None (local MailHog only)</option></select></label>
        <label>SMTP username<input value={config.smtp_username} onChange={(e) => set("smtp_username", e.target.value)} autoComplete="off" /></label>
        <label>SMTP password<input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder={config.smtp_password_configured ? "Saved; leave blank to keep" : "Optional"} autoComplete="new-password" /></label>
        <label>From email<input type="email" required value={config.from_email} onChange={(e) => set("from_email", e.target.value)} /></label>
        <label>From name<input value={config.from_name} onChange={(e) => set("from_name", e.target.value)} /></label>
        <label>Reply-to<input type="email" value={config.reply_to} onChange={(e) => set("reply_to", e.target.value)} placeholder="Optional" /></label>
      </div>
      <h3>Limits</h3>
      <div className="field-grid">
        {([
          ["monthly_limit", "Platform emails / month"], ["tenant_daily_limit", "Workspace emails / day"],
          ["tenant_monthly_limit", "Workspace emails / month"], ["max_broadcast_recipients", "Recipients / announcement"],
          ["broadcasts_per_day", "Announcements / workspace / day"], ["broadcasts_per_week", "Announcements / workspace / week"],
        ] as const).map(([key, label]) => <label key={key}>{label}<input type="number" min={key === "broadcasts_per_day" ? 0 : 1} value={config[key]} onChange={(e) => set(key, Number(e.target.value))} /></label>)}
      </div>
      <button className="primary" type="submit" disabled={busy || !config.config_key_available}>Save settings</button>
    </form>
    <div className="toolbar" style={{ marginTop: 20 }}>
      <label>Test recipient<input type="email" value={recipient} onChange={(e) => setRecipient(e.target.value)} placeholder="you@example.com" /></label>
      <button type="button" disabled={busy || !recipient} onClick={() => void run(() => client.request<EmailConfig>("/api/admin/platform/email/test", { method: "POST", withTenant: false, body: JSON.stringify({ recipient }) }), "Test email sent. Check the inbox before enabling." )}>Send test</button>
      <button type="button" disabled={busy || (config.status !== "tested" && config.status !== "enabled")} onClick={() => void run(() => client.request<EmailConfig>("/api/admin/platform/email/enabled", { method: "POST", withTenant: false, body: JSON.stringify({ enabled: config.status !== "enabled" }) }), config.status === "enabled" ? "Email disabled." : "Email enabled." )}>{config.status === "enabled" ? "Disable email" : "Enable email"}</button>
    </div>
    {config.tested_at ? <p className="muted">Last successful test: {new Date(config.tested_at).toLocaleString()}</p> : null}
    <section style={{ marginTop: 24 }}>
      <h3>Blocked recipients</h3>
      <p className="muted">Use this list for addresses that bounced, complained, or asked to stop all mail. Queued messages to a blocked address are cancelled.</p>
      <form className="toolbar" onSubmit={async (event) => { event.preventDefault(); setBusy(true); setError(""); try {
        await client.request<void>("/api/admin/platform/email/suppression", { method: "POST", withTenant: false, body: JSON.stringify({ recipient: blockedAddress, reason: blockedReason }) });
        setSuppressed(await client.request<SuppressedEmail[]>("/api/admin/platform/email/suppression", { withTenant: false }));
        setBlockedAddress(""); setBlockedReason("");
      } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not block recipient."); } finally { setBusy(false); } }}>
        <input type="email" required placeholder="person@example.com" value={blockedAddress} onChange={(e) => setBlockedAddress(e.target.value)} />
        <input required minLength={3} maxLength={200} placeholder="Reason" value={blockedReason} onChange={(e) => setBlockedReason(e.target.value)} />
        <button disabled={busy}>Block address</button>
      </form>
      {suppressed.length ? <ul>{suppressed.map((entry) => <li key={entry.recipient}>{entry.recipient} · {entry.reason} <button type="button" disabled={busy} onClick={async () => { setBusy(true); setError(""); try {
        await client.request<void>(`/api/admin/platform/email/suppression?recipient=${encodeURIComponent(entry.recipient)}`, { method: "DELETE", withTenant: false });
        setSuppressed((rows) => rows.filter((row) => row.recipient !== entry.recipient));
      } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not unblock recipient."); } finally { setBusy(false); } }}>Unblock</button></li>)}</ul> : <p className="muted">No blocked addresses.</p>}
    </section>
    {notice ? <p role="status" className="notice">{notice}</p> : null}
    {error ? <p role="alert" className="notice notice--error">{error}</p> : null}
  </Panel>;
}
