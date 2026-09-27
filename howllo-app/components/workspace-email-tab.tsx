"use client";

import { useEffect, useState } from "react";
import { API_BASE_URL } from "@/lib/config";
import { readStoredBearerToken } from "@/components/dev-auth-panel";

type Settings = {
  enabled: boolean; reply_notifications: boolean; important_updates: boolean;
  digest_enabled: boolean; broadcast_enabled: boolean;
  daily_limit: number | null; monthly_limit: number | null;
  effective_daily_limit: number; effective_monthly_limit: number;
  used_today: number; used_month: number;
};

export function WorkspaceEmailTab({ tenant }: { tenant: string }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const endpoint = `${API_BASE_URL}/api/admin/tenant-email?tenant_slug=${encodeURIComponent(tenant)}`;
  const token = () => readStoredBearerToken(tenant).trim();
  async function request(url: string, method = "GET", payload?: object) {
    const response = await fetch(url, { method, cache: "no-store", headers: { Authorization: token(), "content-type": "application/json" }, body: payload ? JSON.stringify(payload) : undefined });
    if (!response.ok) throw new Error((await response.text()) || `Request failed (${response.status})`);
    return response.json();
  }
  useEffect(() => { let active = true; request(endpoint).then((value: Settings) => { if (active) setSettings(value); }).catch((cause) => { if (active) setError(cause instanceof Error ? cause.message : "Could not load email settings."); }); return () => { active = false; }; }, [tenant]);
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (!settings) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const updated = await request(endpoint, "PUT", {
        enabled: settings.enabled, reply_notifications: settings.reply_notifications,
        important_updates: settings.important_updates, digest_enabled: settings.digest_enabled,
        broadcast_enabled: settings.broadcast_enabled, daily_limit: settings.daily_limit,
        monthly_limit: settings.monthly_limit,
      }) as Settings;
      setSettings(updated); setNotice("Email settings saved.");
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not save settings."); }
    finally { setBusy(false); }
  }
  async function announce(event: React.FormEvent) {
    event.preventDefault(); setBusy(true); setError(""); setNotice("");
    try {
      const result = await request(`${API_BASE_URL}/api/admin/tenant-email/broadcast?tenant_slug=${encodeURIComponent(tenant)}`, "POST", { subject, body }) as { queued: number; eligible: number };
      setSubject(""); setBody(""); setNotice(`Announcement queued for ${result.queued} of ${result.eligible} opted-in members.`);
      setSettings(await request(endpoint) as Settings);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not send announcement."); }
    finally { setBusy(false); }
  }
  if (!settings) return <section className="panel"><p>{error || "Loading email settings…"}</p></section>;
  const toggle = (key: "enabled" | "reply_notifications" | "important_updates" | "digest_enabled" | "broadcast_enabled", label: string) => <label className="manage-row" key={key}><span>{label}</span><input type="checkbox" checked={settings[key]} onChange={(event) => setSettings({ ...settings, [key]: event.target.checked })} /></label>;
  return <div className="grid">
    <section className="panel">
      <h2 className="section-title">Workspace email</h2>
      <p className="section-subtitle">Email is provided by the platform. Members choose which messages they receive.</p>
      <form onSubmit={(event) => void save(event)} className="field-grid" style={{ marginTop: "1rem" }}>
        {toggle("enabled", "Enable email for this workspace")}
        {toggle("reply_notifications", "Replies to posts")}
        {toggle("important_updates", "Important post updates")}
        {toggle("digest_enabled", "Daily digest")}
        {toggle("broadcast_enabled", "Announcements")}
        <label className="field-label">Daily limit (current {settings.effective_daily_limit})<input className="field" type="number" min="1" value={settings.daily_limit ?? ""} placeholder="Platform default" onChange={(e) => setSettings({ ...settings, daily_limit: e.target.value ? Number(e.target.value) : null })} /></label>
        <label className="field-label">Monthly limit (current {settings.effective_monthly_limit})<input className="field" type="number" min="1" value={settings.monthly_limit ?? ""} placeholder="Platform default" onChange={(e) => setSettings({ ...settings, monthly_limit: e.target.value ? Number(e.target.value) : null })} /></label>
        <p className="section-subtitle">Usage: {settings.used_today} / {settings.effective_daily_limit} today · {settings.used_month} / {settings.effective_monthly_limit} this month.</p>
        <button className="button button--cta" disabled={busy}>Save email settings</button>
      </form>
    </section>
    {settings.enabled && settings.broadcast_enabled ? <section className="panel">
      <h2 className="section-title">Send an announcement</h2>
      <p className="section-subtitle">Only members who explicitly opted in will receive it. Platform sending limits apply.</p>
      <form onSubmit={(event) => void announce(event)} className="field-grid" style={{ marginTop: "1rem" }}>
        <label className="field-label">Subject<input className="field" maxLength={180} required value={subject} onChange={(e) => setSubject(e.target.value)} /></label>
        <label className="field-label">Message<textarea className="field" maxLength={10000} required value={body} onChange={(e) => setBody(e.target.value)} rows={7} /></label>
        <button className="button button--cta" disabled={busy}>Queue announcement</button>
      </form>
    </section> : null}
    {notice ? <p className="success-text" role="status">{notice}</p> : null}
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </div>;
}
