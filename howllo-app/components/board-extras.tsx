"use client";

import { useEffect, useState } from "react";
import { API_BASE_URL } from "@/lib/config";
import { readStoredBearerToken } from "@/components/dev-auth-panel";

type Presentation = { announcement: string; sidebar_text: string; footer_text: string };
type Plugin = { id: string; name: string; description: string; slot: string; enabled: boolean };
const EMPTY: Presentation = { announcement: "", sidebar_text: "", footer_text: "" };

export function BoardExtras({ tenant, boardId }: { tenant: string; boardId: string }) {
  const [values, setValues] = useState<Presentation>(EMPTY);
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const base = `${API_BASE_URL}/api/admin/boards/${encodeURIComponent(boardId)}`;
  const headers = () => ({ Authorization: readStoredBearerToken(tenant).trim() });

  useEffect(() => {
    let active = true;
    Promise.all([
      fetch(`${base}/presentation`, { headers: headers(), cache: "no-store" }),
      fetch(`${base}/plugins`, { headers: headers(), cache: "no-store" }),
    ]).then(async ([settings, catalog]) => {
      if (!settings.ok || !catalog.ok) throw new Error("Could not load board appearance.");
      const [nextValues, nextPlugins] = await Promise.all([settings.json() as Promise<Presentation>, catalog.json() as Promise<Plugin[]>]);
      if (active) { setValues(nextValues); setPlugins(nextPlugins); }
    }).catch((cause) => { if (active) setError(cause instanceof Error ? cause.message : "Could not load board appearance."); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [base, tenant]);

  async function save() {
    setBusy(true); setError(""); setMessage("");
    try {
      const response = await fetch(`${base}/presentation`, { method: "PUT", headers: { ...headers(), "content-type": "application/json" }, body: JSON.stringify(values) });
      if (!response.ok) throw new Error("Could not save board appearance.");
      setMessage("Board appearance saved.");
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not save."); }
    finally { setBusy(false); }
  }

  async function toggle(plugin: Plugin) {
    setBusy(true); setError(""); setMessage("");
    try {
      const response = await fetch(`${base}/plugins/${encodeURIComponent(plugin.id)}`, { method: "PUT", headers: { ...headers(), "content-type": "application/json" }, body: JSON.stringify({ enabled: !plugin.enabled }) });
      if (!response.ok) throw new Error("Could not update plugin.");
      const catalog = await fetch(`${base}/plugins`, { headers: headers(), cache: "no-store" });
      if (!catalog.ok) throw new Error("Could not refresh plugins.");
      setPlugins(await catalog.json() as Plugin[]);
      setMessage(`${plugin.name} ${plugin.enabled ? "disabled" : "enabled"} on this board.`);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not update plugin."); }
    finally { setBusy(false); }
  }

  return <details className="board-extras">
    <summary>Board appearance and plugins</summary>
    {loading ? <p className="section-subtitle">Loading board appearance…</p> : null}
    <div className="manage-fields" style={{ marginTop: "1rem" }}>
      <p className="section-subtitle">Optional text appears only on this board. Plain text keeps it safe and easy to read.</p>
      <label className="manage-label">Announcement above topics<textarea className="manage-input" rows={2} maxLength={1000} value={values.announcement} onChange={(event) => setValues({ ...values, announcement: event.target.value })} placeholder="A short notice for visitors" /></label>
      <label className="manage-label">Sidebar information<textarea className="manage-input" rows={3} maxLength={1000} value={values.sidebar_text} onChange={(event) => setValues({ ...values, sidebar_text: event.target.value })} placeholder="About this board or posting guidelines" /></label>
      <label className="manage-label">Footer text<textarea className="manage-input" rows={2} maxLength={1000} value={values.footer_text} onChange={(event) => setValues({ ...values, footer_text: event.target.value })} placeholder="A brief note below the topic list" /></label>
      <button type="button" className="button" disabled={busy || loading} onClick={() => void save()}>Save appearance</button>
    </div>
    {plugins.length > 0 ? <div className="board-extras__plugins"><h3 className="section-title">Optional plugins</h3><p className="section-subtitle">Built into Howllo and approved by the platform admin. Each setting applies only to this board.</p>
      {plugins.map((plugin) => <div className="manage-row" key={plugin.id} style={{ justifyContent: "space-between", gap: "1rem" }}><div><strong>{plugin.name}</strong><p className="section-subtitle">{plugin.description}</p></div><button type="button" className="ghost-button" disabled={busy} onClick={() => void toggle(plugin)}>{plugin.enabled ? "Disable" : "Enable"}</button></div>)}
    </div> : null}
    {message ? <p className="success-text" role="status">{message}</p> : null}
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </details>;
}
