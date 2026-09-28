"use client";

import { useEffect, useState } from "react";
import { getWorkspacePolicy, saveWorkspacePolicy } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import type { PolicyLimits, PolicyOverrides, WorkspacePolicyView } from "@/lib/types";

type LimitKey = keyof PolicyLimits;
type RuleKey = "ip_allowlist" | "ip_blocklist" | "blocked_countries";
type RuleDraft = Record<RuleKey, string>;

function errorMessage(error: unknown): string {
  if (!(error instanceof Error)) return "Could not save settings.";
  try { return (JSON.parse(error.message) as { error?: { message?: string } }).error?.message || error.message; }
  catch { return error.message; }
}

const parseRules = (value: string) => value.split(/\r?\n|,/).map((item) => item.trim()).filter(Boolean);

function LimitControl({ label, limitKey, form, view, onChange }: {
  label: string;
  limitKey: LimitKey;
  form: PolicyOverrides;
  view: WorkspacePolicyView;
  onChange: (key: LimitKey, value: number | null) => void;
}) {
  const custom = form[limitKey] !== null;
  const cap = limitKey === "storage_mb" ? Math.min(view.caps.storage_mb, view.storage_cap_mb ?? view.caps.storage_mb) : view.caps[limitKey];
  return <div className="security-limit">
    <label htmlFor={`security-${limitKey}`}>{label}</label>
    <span className="security-limit__field">
      <input id={`security-${limitKey}`} type="number" min="1" max={cap} inputMode="numeric" value={form[limitKey] ?? ""}
        placeholder={`Default ${view.defaults[limitKey]}`} onChange={(event) => onChange(limitKey, event.target.value === "" ? null : Number(event.target.value))} />
      {custom ? <button type="button" onClick={() => onChange(limitKey, null)} aria-label={`Reset ${label.toLowerCase()} to platform default`}>Reset</button> : null}
    </span>
    <small>{custom ? `Default ${view.defaults[limitKey]} · max ${cap}` : `Maximum ${cap}`}</small>
  </div>;
}

export function SecurityTab({ tenant }: { tenant: string }) {
  const [view, setView] = useState<WorkspacePolicyView | null>(null);
  const [form, setForm] = useState<PolicyOverrides | null>(null);
  const [rules, setRules] = useState<RuleDraft>({ ip_allowlist: "", ip_blocklist: "", blocked_countries: "" });
  const [accessOpen, setAccessOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  useEffect(() => {
    let active = true;
    getWorkspacePolicy(tenant, readStoredBearerToken(tenant).trim())
      .then((result) => {
        if (!active) return;
        setView(result);
        setForm(result.overrides);
        setRules({
          ip_allowlist: result.overrides.ip_allowlist.join("\n"),
          ip_blocklist: result.overrides.ip_blocklist.join("\n"),
          blocked_countries: result.overrides.blocked_countries.join("\n"),
        });
        setAccessOpen(Boolean(result.overrides.ip_allowlist.length || result.overrides.ip_blocklist.length || result.overrides.blocked_countries.length));
      })
      .catch((cause) => { if (active) setError(errorMessage(cause)); });
    return () => { active = false; };
  }, [tenant]);

  const input = form ? {
    ...form,
    ip_allowlist: parseRules(rules.ip_allowlist),
    ip_blocklist: parseRules(rules.ip_blocklist),
    blocked_countries: parseRules(rules.blocked_countries),
  } : null;

  async function save(event: React.FormEvent) {
    event.preventDefault();
    if (!input) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const next = await saveWorkspacePolicy(tenant, readStoredBearerToken(tenant).trim(), input);
      setView(next); setForm(next.overrides);
      setRules({ ip_allowlist: next.overrides.ip_allowlist.join("\n"), ip_blocklist: next.overrides.ip_blocklist.join("\n"), blocked_countries: next.overrides.blocked_countries.join("\n") });
      setNotice("Security settings saved.");
    } catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(false); }
  }

  if (!view || !form || !input) return <section className="panel">{error || "Loading settings…"}</section>;

  const setLimit = (key: LimitKey, value: number | null) => { setForm({ ...form, [key]: value }); setNotice(""); };
  const setRule = (key: RuleKey, value: string) => { setRules({ ...rules, [key]: value }); setNotice(""); };
  const effective = (key: LimitKey) => form[key] ?? view.defaults[key];
  const postsError = effective("posts_per_day") < effective("posts_per_hour") ? "Daily posts must be at least the hourly limit." : "";
  const commentsError = effective("comments_per_day") < effective("comments_per_hour") ? "Daily comments must be at least the hourly limit." : "";
  const usedMb = view.storage_used_bytes / 1024 / 1024;
  const storageMb = effective("storage_mb");
  const storageError = storageMb < usedMb ? "The limit must be higher than the storage already used." : "";
  const hasRules = Boolean(input.ip_allowlist.length || input.ip_blocklist.length || input.blocked_countries.length);
  const dirty = JSON.stringify(input) !== JSON.stringify(view.overrides);
  const limitProps = { form, view, onChange: setLimit };

  return <form className="security-page" onSubmit={(event) => void save(event)}>
    <section className="security-section">
      <div className="security-section__head"><div><h2>Member activity</h2><p>Limits for each person across this workspace.</p></div></div>
      <div className="security-setting-row"><div><strong>Posts</strong><p>New topics from one member</p></div><div className="security-setting-row__controls"><LimitControl label="Per hour" limitKey="posts_per_hour" {...limitProps} /><LimitControl label="Per day" limitKey="posts_per_day" {...limitProps} /></div></div>
      {postsError ? <p className="security-field-error" role="alert">{postsError}</p> : null}
      <div className="security-setting-row"><div><strong>Comments</strong><p>Replies from one member</p></div><div className="security-setting-row__controls"><LimitControl label="Per hour" limitKey="comments_per_hour" {...limitProps} /><LimitControl label="Per day" limitKey="comments_per_day" {...limitProps} /></div></div>
      {commentsError ? <p className="security-field-error" role="alert">{commentsError}</p> : null}
    </section>

    <section className="security-section">
      <div className="security-section__head"><div><h2>Board activity</h2><p>New activity pauses briefly when a board receives a burst of posts or comments.</p></div></div>
      <div className="security-setting-row"><div><strong>Slowdown threshold</strong><p>Across one board in 10 minutes</p></div><div className="security-setting-row__controls"><LimitControl label="Posts" limitKey="board_posts_per_10m" {...limitProps} /><LimitControl label="Comments" limitKey="board_comments_per_10m" {...limitProps} /></div></div>
    </section>

    <section className="security-section">
      <div className="security-section__head"><div><h2>Storage</h2><p>{usedMb.toFixed(1)} MB used in this workspace.</p></div></div>
      <div className="security-setting-row"><div><strong>Workspace storage</strong><p>Maximum space for uploads</p></div><div className="security-setting-row__controls"><LimitControl label="Limit (MB)" limitKey="storage_mb" {...limitProps} /></div></div>
      {storageError ? <p className="security-field-error" role="alert">{storageError}</p> : null}
    </section>

    <details className="security-section security-access" open={accessOpen} onToggle={(event) => setAccessOpen(event.currentTarget.open)}>
      <summary><span><strong>IP and country rules</strong><small>{hasRules ? "Rules enabled" : "No restrictions"}</small></span><span aria-hidden="true">⌄</span></summary>
      <div className="security-access__fields">
        <p>These rules apply to posts, comments, and uploads. Enter one value per line.</p>
        <label>Allowed IPs <span className="security-help" title="When set, only these IPs can post, comment, and upload." tabIndex={0}>?</span><textarea rows={3} value={rules.ip_allowlist} onChange={(event) => setRule("ip_allowlist", event.target.value)} placeholder="Leave empty to allow everyone" /></label>
        <label>Blocked IPs<textarea rows={3} value={rules.ip_blocklist} onChange={(event) => setRule("ip_blocklist", event.target.value)} placeholder="203.0.113.0/24" /></label>
        <label>Blocked countries <span className="security-help" title="Country rules use Cloudflare location data." tabIndex={0}>?</span><textarea rows={2} value={rules.blocked_countries} onChange={(event) => setRule("blocked_countries", event.target.value)} placeholder="US, GB" /></label>
      </div>
    </details>

    <div className="security-actions"><span>{dirty ? "Unsaved changes" : "Using saved settings"}</span><button className="button" type="submit" disabled={busy || !dirty || Boolean(postsError || commentsError || storageError)}>{busy ? "Saving…" : "Save changes"}</button></div>
    {notice ? <p className="success-text" role="status">{notice}</p> : null}
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </form>;
}
