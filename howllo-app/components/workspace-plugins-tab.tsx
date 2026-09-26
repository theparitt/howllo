"use client";

import { useEffect, useState } from "react";
import { API_BASE_URL } from "@/lib/config";
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

  return <div className="page-stack" style={{ maxWidth: 850 }}>
    <section className="panel">
      <h2 className="section-title">Workspace plugins</h2>
      <p className="section-subtitle">Change the public board’s look. Only platform-approved plugins appear here. One plugin can be active per area.</p>
    </section>
    {loading ? <section className="panel">Loading plugins…</section> : plugins.length === 0 ? <section className="panel">No plugins are available yet.</section> : plugins.map((plugin) => <section className="panel" key={plugin.id}>
      <div className="manage-row" style={{ justifyContent: "space-between", gap: "1rem" }}>
        <div><p className="section-subtitle" style={{ margin: 0 }}>{slotName[plugin.slot] ?? plugin.slot} · v{plugin.version}</p><h3 className="section-title" style={{ margin: ".35rem 0" }}>{plugin.name}</h3><p className="section-subtitle">{plugin.description}</p></div>
        <button type="button" className={plugin.enabled ? "button" : "button button--cta"} disabled={busy !== null} onClick={() => void toggle(plugin)}>{plugin.enabled ? "Disable" : "Enable"}</button>
      </div>
    </section>)}
    {notice ? <p role="status" className="success-text">{notice}</p> : null}
    {error ? <p role="alert" className="error-text">{error}</p> : null}
  </div>;
}
