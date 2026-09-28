"use client";

import { useEffect, useState } from "react";
import { getWorkspacePolicy, saveWorkspacePolicy } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import type { PolicyLimits, PolicyOverrides, WorkspacePolicyView } from "@/lib/types";

type LimitKey = keyof PolicyLimits;
type RuleKey = "ip_allowlist" | "ip_blocklist" | "blocked_countries";
type RuleDraft = Record<RuleKey, string>;

function message(error: unknown): string {
  if (!(error instanceof Error)) return "Could not save settings.";
  try { return (JSON.parse(error.message) as { error?: { message?: string } }).error?.message || error.message; }
  catch { return error.message; }
}

function ShieldIcon() {
  return <svg viewBox="0 0 48 48" fill="none" aria-hidden="true"><path d="M24 4 41 10v12c0 11-6.3 18.4-17 22C13.3 40.4 7 33 7 22V10L24 4Z" stroke="currentColor" strokeWidth="2.5" strokeLinejoin="round"/><path d="m16 24 5 5 11-12" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"/></svg>;
}

function LimitControl({ label, limitKey, form, view, onChange }: {
  label: string;
  limitKey: LimitKey;
  form: PolicyOverrides;
  view: WorkspacePolicyView;
  onChange: (key: LimitKey, value: number | null) => void;
}) {
  const custom = form[limitKey] !== null;
  const cap = limitKey === "storage_mb" ? Math.min(view.caps.storage_mb, view.storage_cap_mb ?? view.caps.storage_mb) : view.caps[limitKey];
  return <div className={`security-limit${custom ? " security-limit--custom" : ""}`}>
    <div className="security-limit__top"><label htmlFor={`security-${limitKey}`}>{label}</label><span>{custom ? "Custom" : "Default"}</span></div>
    <div className="security-limit__input-row">
      <input id={`security-${limitKey}`} type="number" min="1" max={cap} inputMode="numeric" value={form[limitKey] ?? ""}
        placeholder={String(view.defaults[limitKey])} onChange={(event) => onChange(limitKey, event.target.value === "" ? null : Number(event.target.value))} />
      {custom ? <button type="button" onClick={() => onChange(limitKey, null)} aria-label={`Use platform default for ${label.toLowerCase()}`} title="Use platform default">↺</button> : null}
    </div>
    <small>{custom ? `Platform default ${view.defaults[limitKey]}` : `Up to ${cap}`}</small>
  </div>;
}

const parseRules = (value: string) => value.split(/\r?\n|,/).map((item) => item.trim()).filter(Boolean);

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
      .then((result) => { if (active) { setView(result); setForm(result.overrides); setRules({ ip_allowlist: result.overrides.ip_allowlist.join("\n"), ip_blocklist: result.overrides.ip_blocklist.join("\n"), blocked_countries: result.overrides.blocked_countries.join("\n") }); setAccessOpen(Boolean(result.overrides.ip_allowlist.length || result.overrides.ip_blocklist.length || result.overrides.blocked_countries.length)); } })
      .catch((cause) => { if (active) setError(message(cause)); });
    return () => { active = false; };
  }, [tenant]);

  const input = form ? { ...form, ip_allowlist: parseRules(rules.ip_allowlist), ip_blocklist: parseRules(rules.ip_blocklist), blocked_countries: parseRules(rules.blocked_countries) } : null;

  async function save(event: React.FormEvent) {
    event.preventDefault();
    if (!input) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const next = await saveWorkspacePolicy(tenant, readStoredBearerToken(tenant).trim(), input);
      setView(next); setForm(next.overrides);
      setRules({ ip_allowlist: next.overrides.ip_allowlist.join("\n"), ip_blocklist: next.overrides.ip_blocklist.join("\n"), blocked_countries: next.overrides.blocked_countries.join("\n") });
      setNotice("Security settings saved.");
    } catch (cause) { setError(message(cause)); }
    finally { setBusy(false); }
  }

  if (!view || !form || !input) return <section className="panel">{error || "Loading settings…"}</section>;
  const setLimit = (key: LimitKey, value: number | null) => { setForm({ ...form, [key]: value }); setNotice(""); };
  const setRule = (key: RuleKey, value: string) => { setRules({ ...rules, [key]: value }); setNotice(""); };
  const effective = (key: LimitKey) => form[key] ?? view.defaults[key];
  const postsLimitError = effective("posts_per_day") < effective("posts_per_hour") ? "Daily posts must be at least the hourly limit." : "";
  const commentsLimitError = effective("comments_per_day") < effective("comments_per_hour") ? "Daily comments must be at least the hourly limit." : "";
  const usedMb = view.storage_used_bytes / 1024 / 1024;
  const storageMb = effective("storage_mb");
  const storagePercent = Math.min(100, Math.max(usedMb > 0 ? 1 : 0, Math.round((usedMb / Math.max(storageMb, 1)) * 100)));
  const storageLimitError = storageMb < usedMb ? "The limit must be higher than the storage already used." : "";
  const hasAccessRules = Boolean(input.ip_allowlist.length || input.ip_blocklist.length || input.blocked_countries.length);
  const dirty = JSON.stringify(input) !== JSON.stringify(view.overrides);
  const limitProps = { form, view, onChange: setLimit };

  return <section className="security-page" aria-label="Workspace security settings">
    <div className="security-intro">
      <div className="security-intro__copy"><span className="security-intro__eyebrow">WORKSPACE CONTROLS</span><h2>Keep the conversation healthy.</h2><p>Set comfortable limits for real members. Slow down bursts before they turn into spam.</p></div>
      <div className="security-intro__mark"><ShieldIcon /></div>
    </div>

    <div className="security-summary" aria-label="Current workspace limits">
      <div><span>Posts per member</span><strong>{effective("posts_per_hour")}<small> / hour</small></strong></div>
      <div><span>Comments per member</span><strong>{effective("comments_per_hour")}<small> / hour</small></strong></div>
      <div><span>Storage used</span><strong>{usedMb.toFixed(1)}<small> / {storageMb} MB</small></strong></div>
    </div>

    <form className="security-form" onSubmit={(event) => void save(event)}>
      <div className="security-grid">
        <div className="security-grid__main">
          <section className="security-card">
            <div className="security-card__heading"><span className="security-card__icon security-card__icon--blue" aria-hidden="true">✦</span><div><h3>Member limits</h3><p>How often one person can post or comment.</p></div></div>
            <div className="security-group"><div className="security-group__title"><strong>Posts</strong><span>New topics from one member</span></div><div className="security-controls"><LimitControl label="Per hour" limitKey="posts_per_hour" {...limitProps} /><LimitControl label="Per day" limitKey="posts_per_day" {...limitProps} /></div>{postsLimitError ? <p className="security-field-error" role="alert">{postsLimitError}</p> : null}</div>
            <div className="security-group"><div className="security-group__title"><strong>Comments</strong><span>Replies from one member</span></div><div className="security-controls"><LimitControl label="Per hour" limitKey="comments_per_hour" {...limitProps} /><LimitControl label="Per day" limitKey="comments_per_day" {...limitProps} /></div>{commentsLimitError ? <p className="security-field-error" role="alert">{commentsLimitError}</p> : null}</div>
          </section>
          <section className="security-card">
            <div className="security-card__heading"><span className="security-card__icon security-card__icon--coral" aria-hidden="true">↗</span><div><h3>Board slowdown</h3><p>Pause new activity when an entire board gets flooded.</p></div></div>
            <div className="security-controls security-controls--board"><LimitControl label="Posts / 10 min" limitKey="board_posts_per_10m" {...limitProps} /><LimitControl label="Comments / 10 min" limitKey="board_comments_per_10m" {...limitProps} /></div>
          </section>
        </div>
        <div className="security-grid__aside">
          <section className="security-card security-card--storage">
            <div className="security-card__heading"><span className="security-card__icon security-card__icon--mint" aria-hidden="true">▣</span><div><h3>Storage</h3><p>Files and images in this workspace.</p></div></div>
            <div className="security-storage"><div className="security-storage__numbers"><strong>{usedMb.toFixed(1)} MB used</strong><span>{storagePercent}% of {storageMb} MB</span></div><div className="security-storage__track" role="meter" aria-label="Workspace storage used" aria-valuemin={0} aria-valuemax={storageMb} aria-valuenow={Math.min(usedMb, storageMb)}><span style={{ width: `${storagePercent}%` }} /></div></div>
            <LimitControl label="Workspace limit (MB)" limitKey="storage_mb" {...limitProps} />
            {storageLimitError ? <p className="security-field-error" role="alert">{storageLimitError}</p> : null}
          </section>
          <section className="security-card security-card--access">
            <div className="security-card__heading"><span className="security-card__icon security-card__icon--lilac" aria-hidden="true">◎</span><div><h3>Posting access</h3><p>Control who can post, comment, or upload.</p></div></div>
            <details className="security-access" open={accessOpen} onToggle={(event) => setAccessOpen(event.currentTarget.open)}>
              <summary>{hasAccessRules ? "Access rules are on" : "Everyone can participate"}<span>{hasAccessRules ? "Edit rules" : "Add rules"} <span aria-hidden="true">⌄</span></span></summary>
              <div className="security-access__fields">
                <p>Use one IP, network range, or two-letter country code per line.</p>
                <label>Allowed IPs <span title="When set, only these IPs can post, comment, and upload." tabIndex={0} className="security-help">?</span><textarea rows={3} value={rules.ip_allowlist} onChange={(event) => setRule("ip_allowlist", event.target.value)} placeholder="Empty means everyone" /></label>
                <label>Blocked IPs<textarea rows={3} value={rules.ip_blocklist} onChange={(event) => setRule("ip_blocklist", event.target.value)} placeholder="203.0.113.0/24" /></label>
                <label>Blocked countries <span title="Country rules use Cloudflare location data." tabIndex={0} className="security-help">?</span><textarea rows={2} value={rules.blocked_countries} onChange={(event) => setRule("blocked_countries", event.target.value)} placeholder="US, GB" /></label>
              </div>
            </details>
          </section>
        </div>
      </div>
      <div className={`security-save${dirty ? " security-save--dirty" : ""}`}><div><strong>{dirty ? "Unsaved changes" : "Settings are up to date"}</strong><span>Empty limit fields use the platform default.</span></div><button className="button button--cta" type="submit" disabled={busy || !dirty || Boolean(postsLimitError || commentsLimitError || storageLimitError)}>{busy ? "Saving…" : "Save changes"}</button></div>
      {notice ? <p className="security-message security-message--success" role="status">{notice}</p> : null}
      {error ? <p className="security-message security-message--error" role="alert">{error}</p> : null}
    </form>
  </section>;
}
