"use client";

import { useEffect, useState } from "react";
import { API_BASE_URL } from "@/lib/config";
import { pluginCovers } from "../lib/plugin-covers";
import { readStoredBearerToken } from "@/components/dev-auth-panel";

type Plugin = { id: string; version: string; name: string; description: string; slot: string; stylesheet_path: string; enabled: boolean };
const slotName: Record<string, string> = {
  "workspace.typography": "Typography",
  "board.directory.layout": "Board directory",
};

export function WorkspacePluginsTab({ tenant }: { tenant: string }) {
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const endpoint = `${API_BASE_URL}/api/tenants/${encodeURIComponent(tenant)}/plugins`;
  const token = () => readStoredBearerToken(tenant).trim();

  async function load() {
    const response = await fetch(endpoint, { headers: { Authorization: token() }, cache: "no-store" });
    if (!response.ok) throw new Error("Could not load plugins.");
    setPlugins(await response.json() as Plugin[]);
  }
  useEffect(() => {
    let active = true;
    setLoading(true);
    fetch(endpoint, { headers: { Authorization: token() }, cache: "no-store" })
      .then(async (response) => { if (!response.ok) throw new Error("Could not load plugins."); return response.json() as Promise<Plugin[]>; })
      .then((items) => { if (active) setPlugins(items); })
      .catch((cause) => { if (active) setError(cause instanceof Error ? cause.message : "Could not load plugins."); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tenant]);

  async function toggle(plugin: Plugin) {
    const replaced = !plugin.enabled ? plugins.find((item) => item.slot === plugin.slot && item.enabled && item.id !== plugin.id) : null;
    if (replaced && !window.confirm(`Enable ${plugin.name}? This will turn off ${replaced.name} for ${slotName[plugin.slot] ?? plugin.slot}.`)) return;
    setBusy(plugin.id); setError(""); setNotice("");
    try {
      const response = await fetch(`${endpoint}/${encodeURIComponent(plugin.id)}`, {
        method: "PUT", headers: { "content-type": "application/json", Authorization: token() },
        body: JSON.stringify({ enabled: !plugin.enabled }),
      });
      if (!response.ok) throw new Error(`Could not ${plugin.enabled ? "disable" : "enable"} plugin.`);
      await load();
      setNotice(`${plugin.name} ${plugin.enabled ? "disabled" : "enabled"}.`);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not update plugin."); }
    finally { setBusy(null); }
  }

  return <div className="page-stack workspace-plugin-list">
    <p className="section-subtitle">Optional features supplied by Howllo. One choice can be active in each style slot.</p>
    {loading ? <section className="panel">Loading plugins…</section> : plugins.length === 0 ? <section className="panel"><h2 className="section-title">No plugins available</h2><p className="section-subtitle">Approved workspace plugins will appear here.</p></section> : <div className="workspace-plugin-grid">{plugins.map((plugin) => <section className="panel workspace-plugin-card" key={plugin.id}>
      {pluginCovers[plugin.id] ? <img className="workspace-plugin-card__cover" src={pluginCovers[plugin.id]} alt="" width="1080" height="360" loading="lazy" decoding="async" /> : null}
      <div className="workspace-plugin-card__body"><p className="section-subtitle">{slotName[plugin.slot] ?? plugin.slot} · v{plugin.version}</p><h3 className="section-title">{plugin.name}</h3><p className="section-subtitle">{plugin.description}</p></div>
      <div className="workspace-plugin-card__footer"><span className="member-row__status" data-status={plugin.enabled ? "active" : "off"}>{plugin.enabled ? "Enabled" : "Off"}</span><button type="button" className="ghost-button" disabled={busy !== null} onClick={() => void toggle(plugin)}>{busy === plugin.id ? "Saving…" : plugin.enabled ? "Disable" : "Enable"}</button></div>
    </section>)}</div>}
    {notice ? <p role="status" className="success-text">{notice}</p> : null}
    {error ? <p role="alert" className="error-text">{error}</p> : null}
  </div>;
}
