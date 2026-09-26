import { useEffect, useState } from "react";
import { useSession } from "../../lib/session";
import { Panel } from "../../components/Panel";

type Plugin = { id: string; version: string; name: string; description: string; slot: string; stylesheet_path: string; is_approved: boolean };

export function PluginCatalogTab() {
  const { client } = useSession();
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");

  useEffect(() => {
    let active = true;
    client.request<Plugin[]>("/api/admin/plugins", { withTenant: false })
      .then((items) => { if (active) setPlugins(items); })
      .catch((cause) => { if (active) setError(cause instanceof Error ? cause.message : "Could not load plugins."); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [client]);

  async function toggle(plugin: Plugin) {
    setBusy(plugin.id); setError(""); setMessage("");
    try {
      await client.request<void>(`/api/admin/plugins/${encodeURIComponent(plugin.id)}`, {
        method: "PATCH", withTenant: false,
        body: JSON.stringify({ enabled: !plugin.is_approved }),
      });
      setPlugins((items) => items.map((item) => item.id === plugin.id ? { ...item, is_approved: !item.is_approved } : item));
      setMessage(`${plugin.name} ${plugin.is_approved ? "disabled" : "approved"}.`);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not update plugin."); }
    finally { setBusy(null); }
  }

  return <div style={{ display: "grid", gap: "1rem" }}>
    <Panel><h2>Approved plugins</h2><p className="muted">Only approved packages can be enabled by workspaces. Plugin files are shipped with Howllo and reviewed before they appear here.</p></Panel>
    {loading ? <Panel>Loading plugins…</Panel> : plugins.map((plugin) => <Panel key={plugin.id}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16 }}>
        <div><strong>{plugin.name}</strong> <span className="muted">v{plugin.version} · {plugin.slot}</span><p className="muted">{plugin.description}</p></div>
        <button type="button" disabled={busy !== null} onClick={() => void toggle(plugin)}>{plugin.is_approved ? "Disable" : "Approve"}</button>
      </div>
    </Panel>)}
    {message ? <p role="status">{message}</p> : null}
    {error ? <p role="alert">{error}</p> : null}
  </div>;
}
