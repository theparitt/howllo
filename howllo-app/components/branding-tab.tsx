"use client";

import { useCallback, useEffect, useState } from "react";
import { getTenantBranding, managerUpdateTenantBranding, uploadImage } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import { prepareBrandImage } from "../lib/prepare-brand-image";
import { contrastInk } from "@/lib/theme";

const PRESETS = [
  { name: "Coral", accent: "#e0522f", background: "#fff3ec" },
  { name: "Ocean", accent: "#186789", background: "#eaf5f8" },
  { name: "Forest", accent: "#24735c", background: "#eff7ef" },
  { name: "Violet", accent: "#674bac", background: "#f3effb" },
];
const COLOR = /^#[0-9a-fA-F]{6}$/;

export function BrandingTab({ tenant }: { tenant: string }) {
  const [siteName, setSiteName] = useState("");
  const [logoUrl, setLogoUrl] = useState<string | null>(null);
  const [accent, setAccent] = useState("#e0522f");
  const [background, setBackground] = useState("#fff3ec");
  const [showRoadmap, setShowRoadmap] = useState(true);
  const [showBoards, setShowBoards] = useState(true);
  const [showFeed, setShowFeed] = useState(true);
  const [showPoweredBy, setShowPoweredBy] = useState(true);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  const load = useCallback(async () => {
    try {
      const branding = await getTenantBranding(tenant);
      setSiteName(branding.site_name);
      setLogoUrl(branding.logo_url);
      setAccent(branding.accent_color ?? "#e0522f");
      setBackground(branding.background_color ?? "#fff3ec");
      setShowRoadmap(branding.show_roadmap);
      setShowBoards(branding.show_boards);
      setShowFeed(branding.show_feed);
      setShowPoweredBy(branding.show_powered_by);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not load public site branding.");
    } finally { setLoading(false); }
  }, [tenant]);
  useEffect(() => { void load(); }, [load]);

  const chooseLogo = async (file: File | undefined) => {
    if (!file) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const prepared = await prepareBrandImage(file, 1200, 360);
      const url = await uploadImage(prepared, readStoredBearerToken(tenant).trim());
      setLogoUrl(url);
      setNotice("Logo uploaded. Save changes to publish it.");
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not upload logo."); }
    finally { setBusy(false); }
  };

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!siteName.trim() || siteName.trim().length > 120) { setError("Enter a public site name of 1–120 characters."); return; }
    if (!COLOR.test(accent) || !COLOR.test(background)) { setError("Use six-digit colors such as #186789."); return; }
    setBusy(true); setError(""); setNotice("");
    try {
      await managerUpdateTenantBranding(tenant, readStoredBearerToken(tenant).trim(), {
        site_name: siteName.trim(), logo_url: logoUrl, accent_color: accent,
        background_color: background, show_boards: showBoards, show_feed: showFeed,
        show_roadmap: showRoadmap, show_powered_by: showPoweredBy,
      });
      setNotice("Public site branding saved. Refresh the public Web to see the change.");
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not save branding."); }
    finally { setBusy(false); }
  };

  return <section className="panel">
    <h2 className="section-title">Public site branding</h2>
    <p className="section-subtitle">These settings change the end-user Web for this workspace. Howllo App keeps its own staff identity.</p>
    {loading ? <p>Loading…</p> : <form className="manage-fields" style={{ marginTop: "1rem" }} onSubmit={(event) => void save(event)}>
      <label className="manage-label">Public site name
        <input className="manage-input" value={siteName} maxLength={120} onChange={(event) => setSiteName(event.target.value)} required />
      </label>
      <div className="manage-label">Public logo
        <div className="branding-logo-preview" style={{ background }}>
          {logoUrl ? <img src={logoUrl} alt="Current public logo" /> : <strong style={{ color: accent }}>{siteName || "Your workspace"}</strong>}
        </div>
        <input className="manage-input" type="file" accept="image/png,image/jpeg,image/webp" disabled={busy} onChange={(event) => {
          const file = event.target.files?.[0]; event.target.value = ""; void chooseLogo(file);
        }} />
        <span className="section-subtitle">PNG, JPG or WebP · up to 5 MB and 4096 × 4096 pixels. Large images are resized automatically.</span>
        {logoUrl ? <button className="ghost-button" type="button" disabled={busy} onClick={() => { setLogoUrl(null); setNotice("Save changes to remove the logo."); }}>Remove logo</button> : null}
      </div>
      <div className="branding-presets" aria-label="Color themes">{PRESETS.map((preset) => <button
        key={preset.name} type="button" className="branding-preset" disabled={busy}
        onClick={() => { setAccent(preset.accent); setBackground(preset.background); }}
        aria-label={`Use ${preset.name} theme`}
      ><span style={{ background: preset.accent }} />{preset.name}</button>)}</div>
      <label className="manage-label">Accent color
        <div className="branding-color-field"><input type="color" value={COLOR.test(accent) ? accent : "#e0522f"} onChange={(event) => setAccent(event.target.value)} /><input className="manage-input" value={accent} onChange={(event) => setAccent(event.target.value)} maxLength={7} /></div>
      </label>
      <label className="manage-label">Page background
        <div className="branding-color-field"><input type="color" value={COLOR.test(background) ? background : "#fff3ec"} onChange={(event) => setBackground(event.target.value)} /><input className="manage-input" value={background} onChange={(event) => setBackground(event.target.value)} maxLength={7} /></div>
      </label>
      <div className="manage-fields" style={{ gap: ".55rem" }}>
        <label className="manage-check"><input type="checkbox" checked={showBoards} onChange={(event) => setShowBoards(event.target.checked)} /> Show board directory ({`/${tenant}`})</label>
        <label className="manage-check"><input type="checkbox" checked={showFeed} onChange={(event) => setShowFeed(event.target.checked)} /> Show workspace feed ({`/${tenant}/feed`})</label>
        <label className="manage-check"><input type="checkbox" checked={showRoadmap} onChange={(event) => setShowRoadmap(event.target.checked)} /> Show roadmap ({`/${tenant}/roadmap`})</label>
      </div>
      <p className="section-subtitle">Each board also has its own URL. Turn an individual board on or off in the Boards tab.</p>
      <label className="manage-check"><input type="checkbox" checked={showPoweredBy} onChange={(event) => setShowPoweredBy(event.target.checked)} /> Show “Powered by Howllo”</label>
      <div className="branding-preview" style={{ background }}>
        <span style={{ color: accent }}>LIVE PREVIEW</span>
        <strong>{siteName || "Your workspace"}</strong>
        <button type="button" style={{ background: accent, color: contrastInk(accent) }}>Share feedback</button>
      </div>
      <div className="manage-row__actions"><button className="button button--cta" type="submit" disabled={busy}>{busy ? "Saving…" : "Save public branding"}</button><a className="ghost-button" href={`${(process.env.NEXT_PUBLIC_HOWLLO_PUBLIC_WEB_URL || "http://localhost:7703").replace(/\/$/, "")}/${encodeURIComponent(tenant)}`} target="_blank" rel="noreferrer">Open public Web ↗</a></div>
    </form>}
    {notice ? <p className="success-text" role="status">{notice}</p> : null}
    {error ? <p className="error-text" role="alert">{error}</p> : null}
  </section>;
}
