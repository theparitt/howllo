import { useEffect, useState } from "react";
import { admin } from "@howllo/api-client";
import type { AdminTenantSummary, PlatformPolicy, PolicyLimits, WorkspacePolicy } from "@howllo/types";
import { useSession } from "../../lib/session";
import { Panel } from "../../components/Panel";

const fields: { key: keyof PolicyLimits; label: string }[] = [
  { key: "posts_per_hour", label: "Posts per member / hour" },
  { key: "posts_per_day", label: "Posts per member / day" },
  { key: "comments_per_hour", label: "Comments per member / hour" },
  { key: "comments_per_day", label: "Comments per member / day" },
  { key: "board_posts_per_10m", label: "Posts per board / 10 minutes" },
  { key: "board_comments_per_10m", label: "Comments per board / 10 minutes" },
  { key: "storage_mb", label: "Storage per workspace (MB)" },
];

export function PlatformPolicyTab() {
  const { client } = useSession();
  const api = admin(client);
  const [policy, setPolicy] = useState<PlatformPolicy | null>(null);
  const [tenants, setTenants] = useState<AdminTenantSummary[]>([]);
  const [slug, setSlug] = useState("");
  const [workspace, setWorkspace] = useState<WorkspacePolicy | null>(null);
  const [cap, setCap] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    api.getPlatformPolicy().then(setPolicy).catch((e) => setError(e instanceof Error ? e.message : "Could not load policy."));
    api.listTenants().then((items) => { setTenants(items); if (items[0]) setSlug(items[0].slug); }).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client]);

  useEffect(() => {
    if (!slug) return;
    api.getPlatformWorkspacePolicy(slug).then((data) => { setWorkspace(data); setCap(data.storage_cap_mb?.toString() ?? ""); })
      .catch((e) => setError(e instanceof Error ? e.message : "Could not load workspace quota."));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, slug]);

  async function savePolicy(event: React.FormEvent) {
    event.preventDefault();
    if (!policy) return;
    setBusy(true); setError(""); setMessage("");
    try { setPolicy(await api.savePlatformPolicy(policy)); setMessage("Platform defaults and ceilings saved."); }
    catch (e) { setError(e instanceof Error ? e.message : "Could not save policy."); }
    finally { setBusy(false); }
  }

  async function saveCap(event: React.FormEvent) {
    event.preventDefault();
    if (!slug) return;
    setBusy(true); setError(""); setMessage("");
    try { setWorkspace(await api.saveWorkspaceStorageCap(slug, cap ? Number(cap) : null)); setMessage("Workspace storage cap saved."); }
    catch (e) { setError(e instanceof Error ? e.message : "Could not save storage cap."); }
    finally { setBusy(false); }
  }

  if (!policy) return <Panel>{error || "Loading policy…"}</Panel>;
  return <div style={{ display: "grid", gap: "1rem" }}>
    <Panel>
      <h2>Workspace defaults</h2>
      <p className="muted">New workspaces inherit these values. Tenants can change them up to the ceiling.</p>
      <form onSubmit={(event) => void savePolicy(event)} style={{ display: "grid", gap: ".85rem" }}>
        {fields.map(({ key, label }) => <div key={key} style={{ display: "grid", gridTemplateColumns: "minmax(220px, 1fr) 140px 140px", gap: ".75rem", alignItems: "center" }}>
          <span>{label}</span>
          <label>Default<input type="number" min="1" max={policy.caps[key]} required value={policy.defaults[key]}
            onChange={(event) => setPolicy({ ...policy, defaults: { ...policy.defaults, [key]: Number(event.target.value) } })} /></label>
          <label>Ceiling<input type="number" min={policy.defaults[key]} max="102400" required value={policy.caps[key]}
            onChange={(event) => setPolicy({ ...policy, caps: { ...policy.caps, [key]: Number(event.target.value) } })} /></label>
        </div>)}
        <button type="submit" disabled={busy}>Save defaults</button>
      </form>
    </Panel>
    <Panel>
      <h2>Workspace storage cap</h2>
      <p className="muted">Set a hard storage ceiling for one workspace. Leave empty to use the platform ceiling.</p>
      <form onSubmit={(event) => void saveCap(event)} style={{ display: "grid", gap: ".75rem", maxWidth: 460 }}>
        <select value={slug} onChange={(event) => setSlug(event.target.value)}>{tenants.map((item) => <option key={item.id} value={item.slug}>{item.name}</option>)}</select>
        <label>Cap (MB)<input type="number" min="1" max={policy.caps.storage_mb} value={cap} placeholder={`Platform ceiling: ${policy.caps.storage_mb}`} onChange={(event) => setCap(event.target.value)} /></label>
        {workspace ? <p className="muted">Used: {(workspace.storage_used_bytes / 1024 / 1024).toFixed(1)} MB · Effective limit: {workspace.effective.storage_mb} MB</p> : null}
        <button type="submit" disabled={busy || !slug}>Save workspace cap</button>
      </form>
    </Panel>
    {message ? <p role="status">{message}</p> : null}
    {error ? <p role="alert">{error}</p> : null}
  </div>;
}
