"use client";

import { useEffect, useState } from "react";
import { getWorkspacePolicy, saveWorkspacePolicy } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import type { PolicyLimits, PolicyOverrides, WorkspacePolicyView } from "@/lib/types";

const fields: { key: keyof PolicyLimits; label: string }[] = [
  { key: "posts_per_hour", label: "Posts per member / hour" },
  { key: "posts_per_day", label: "Posts per member / day" },
  { key: "comments_per_hour", label: "Comments per member / hour" },
  { key: "comments_per_day", label: "Comments per member / day" },
  { key: "board_posts_per_10m", label: "Posts per board / 10 minutes" },
  { key: "board_comments_per_10m", label: "Comments per board / 10 minutes" },
  { key: "storage_mb", label: "Storage per workspace (MB)" },
];

function message(error: unknown): string {
  if (!(error instanceof Error)) return "Could not save settings.";
  try { return (JSON.parse(error.message) as { error?: { message?: string } }).error?.message || error.message; }
  catch { return error.message; }
}

export function SecurityTab({ tenant }: { tenant: string }) {
  const [view, setView] = useState<WorkspacePolicyView | null>(null);
  const [form, setForm] = useState<PolicyOverrides | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  useEffect(() => {
    let active = true;
    getWorkspacePolicy(tenant, readStoredBearerToken(tenant).trim())
      .then((result) => { if (active) { setView(result); setForm(result.overrides); } })
      .catch((cause) => { if (active) setError(message(cause)); });
    return () => { active = false; };
  }, [tenant]);

  async function save(event: React.FormEvent) {
    event.preventDefault();
    if (!form) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const next = await saveWorkspacePolicy(tenant, readStoredBearerToken(tenant).trim(), form);
      setView(next); setForm(next.overrides); setNotice("Security settings saved.");
    } catch (cause) { setError(message(cause)); }
    finally { setBusy(false); }
  }

  if (!view || !form) return <section className="panel">{error || "Loading settings…"}</section>;
  return <section className="panel">
    <h2 className="section-title">Security and limits</h2>
    <p className="section-subtitle">Leave a limit empty to use the platform default. Activity spikes pause new posts or comments for a few minutes.</p>
    <form className="manage-fields" style={{ marginTop: "1rem" }} onSubmit={(event) => void save(event)}>
      {fields.map(({ key, label }) => <label key={key}>{label}
        <input className="manage-input" type="number" min="1"
          max={key === "storage_mb" ? Math.min(view.caps.storage_mb, view.storage_cap_mb ?? view.caps.storage_mb) : view.caps[key]}
          placeholder={`Default: ${view.defaults[key]}`}
          value={form[key] ?? ""}
          onChange={(event) => setForm({ ...form, [key]: event.target.value === "" ? null : Number(event.target.value) })} />
      </label>)}
      <p className="section-subtitle">Storage used: {(view.storage_used_bytes / 1024 / 1024).toFixed(1)} MB of {view.effective.storage_mb} MB.</p>
      <h3 className="section-title">Posting access</h3>
      <p className="section-subtitle">These rules apply to posts, comments, and uploads. One IP or CIDR range per line. Country codes use Cloudflare location data.</p>
      {([
        ["ip_allowlist", "Allowed IPs (leave empty for everyone)"],
        ["ip_blocklist", "Blocked IPs"],
        ["blocked_countries", "Blocked countries (two-letter codes)"],
      ] as const).map(([key, label]) => <label key={key}>{label}
        <textarea className="manage-input" rows={3} value={form[key].join("\n")}
          onChange={(event) => setForm({ ...form, [key]: event.target.value.split(/\r?\n|,/).map((s) => s.trim()).filter(Boolean) })} />
      </label>)}
      <button className="button button--cta" type="submit" disabled={busy}>{busy ? "Saving…" : "Save settings"}</button>
    </form>
    {notice ? <p className="success-text" role="status">{notice}</p> : null}
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}
