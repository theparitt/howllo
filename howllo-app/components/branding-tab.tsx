"use client";

import { useCallback, useEffect, useState } from "react";
import { getTenantManagementSettings, managerUpdateTenantBranding, setWorkspacePublication, uploadImage } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import { prepareBrandImage } from "../lib/prepare-brand-image";
import { contrastInk } from "@/lib/theme";
import type { TenantManagementSettings } from "@/lib/types";

const PUBLIC_WEB_URL = (process.env.NEXT_PUBLIC_HOWLLO_PUBLIC_WEB_URL || "http://localhost:7703").replace(/\/$/, "");

const PRESETS = [
  { name: "Coral", accent: "#e0522f", background: "#fff3ec" },
  { name: "Ocean", accent: "#186789", background: "#eaf5f8" },
  { name: "Forest", accent: "#24735c", background: "#eff7ef" },
  { name: "Violet", accent: "#674bac", background: "#f3effb" },
];
const COLOR = /^#[0-9a-fA-F]{6}$/;
const PAGE_ICONS: Record<string, string> = { boards: "M4 5h16v14H4zM4 10h16M10 10v9", feed: "M5 6h14M5 12h14M5 18h9", roadmap: "M4 19V5m0 0h15l-3 4 3 4H4" };
function PageIcon({ page }: { page: string }) { return <svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={PAGE_ICONS[page]} /></svg>; }
type BrandingUpdate = Pick<TenantManagementSettings, "site_name" | "logo_url" | "accent_color" | "background_color" | "show_powered_by" | "show_roadmap" | "show_boards" | "show_feed" | "require_post_approval" | "posts_per_hour" | "comments_per_hour" | "board_posts_per_10m" | "board_comments_per_10m">;

export function BrandingTab({ tenant }: { tenant: string }) {
  const [siteName, setSiteName] = useState("");
  const [logoUrl, setLogoUrl] = useState<string | null>(null);
  const [accent, setAccent] = useState("#e0522f");
  const [background, setBackground] = useState("#fff3ec");
  const [showRoadmap, setShowRoadmap] = useState(true);
  const [isPublished, setIsPublished] = useState(false);
  const [showBoards, setShowBoards] = useState(true);
  const [showFeed, setShowFeed] = useState(true);
  const [showPoweredBy, setShowPoweredBy] = useState(true);
  const [requirePostApproval, setRequirePostApproval] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [loadError, setLoadError] = useState("");
  const [pageError, setPageError] = useState("");
  const [pageNotice, setPageNotice] = useState("");
  const [appearanceError, setAppearanceError] = useState("");
  const [appearanceNotice, setAppearanceNotice] = useState("");
  const [savedPages, setSavedPages] = useState("");
  const [savedAppearance, setSavedAppearance] = useState("");
  const pagesValue = JSON.stringify([showBoards, showFeed, showRoadmap, requirePostApproval]);
  const appearanceValue = JSON.stringify([siteName, logoUrl, accent, background, showPoweredBy]);
  const pagesDirty = Boolean(savedPages) && savedPages !== pagesValue;
  const appearanceDirty = Boolean(savedAppearance) && savedAppearance !== appearanceValue;

  useEffect(() => {
    if (!pagesDirty && !appearanceDirty) return;
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [pagesDirty, appearanceDirty]);

  const load = useCallback(async () => {
    try {
      const branding = await getTenantManagementSettings(tenant, readStoredBearerToken(tenant).trim());
      setSiteName(branding.site_name);
      setLogoUrl(branding.logo_url);
      setAccent(branding.accent_color ?? "#e0522f");
      setBackground(branding.background_color ?? "#fff3ec");
      setShowRoadmap(branding.show_roadmap);
      setIsPublished(branding.is_published);
      setShowBoards(branding.show_boards);
      setShowFeed(branding.show_feed);
      setShowPoweredBy(branding.show_powered_by);
      setRequirePostApproval(branding.require_post_approval);
      setSavedPages(JSON.stringify([branding.show_boards, branding.show_feed, branding.show_roadmap, branding.require_post_approval]));
      setSavedAppearance(JSON.stringify([branding.site_name, branding.logo_url, branding.accent_color ?? "#e0522f", branding.background_color ?? "#fff3ec", branding.show_powered_by]));
    } catch (cause) {
      setLoadError(cause instanceof Error ? cause.message : "Could not load appearance and page settings.");
    } finally { setLoading(false); }
  }, [tenant]);
  useEffect(() => { void load(); }, [load]);

  const chooseLogo = async (file: File | undefined) => {
    if (!file) return;
    setBusy(true); setAppearanceError(""); setAppearanceNotice("");
    try {
      const prepared = await prepareBrandImage(file, 1200, 360);
      const url = await uploadImage(prepared, readStoredBearerToken(tenant).trim(), tenant);
      setLogoUrl(url);
      setAppearanceNotice("Logo uploaded. Save appearance to publish it.");
    } catch (cause) { setAppearanceError(cause instanceof Error ? cause.message : "Could not upload logo."); }
    finally { setBusy(false); }
  };

  const saveChanges = async (changes: Partial<BrandingUpdate>) => {
    const current = await getTenantManagementSettings(tenant, readStoredBearerToken(tenant).trim());
    await managerUpdateTenantBranding(tenant, readStoredBearerToken(tenant).trim(), {
      site_name: current.site_name,
      logo_url: current.logo_url,
      accent_color: current.accent_color,
      background_color: current.background_color,
      show_boards: current.show_boards,
      show_feed: current.show_feed,
      show_roadmap: current.show_roadmap,
      show_powered_by: current.show_powered_by,
      require_post_approval: current.require_post_approval,
      posts_per_hour: current.posts_per_hour,
      comments_per_hour: current.comments_per_hour,
      board_posts_per_10m: current.board_posts_per_10m,
      board_comments_per_10m: current.board_comments_per_10m,
      ...changes,
    });
  };

  const savePages = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true); setPageError(""); setPageNotice("");
    try {
      await saveChanges({ show_boards: showBoards, show_feed: showFeed, show_roadmap: showRoadmap, require_post_approval: requirePostApproval });
      setSavedPages(pagesValue);
      setPageNotice("Settings updated.");
    } catch (cause) { setPageError(cause instanceof Error ? cause.message : "Could not save pages."); }
    finally { setBusy(false); }
  };

  const saveAppearance = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!siteName.trim() || siteName.trim().length > 120) { setAppearanceError("Enter a site name of 1–120 characters."); return; }
    if (!COLOR.test(accent) || !COLOR.test(background)) { setAppearanceError("Use six-digit colors such as #186789."); return; }
    setBusy(true); setAppearanceError(""); setAppearanceNotice("");
    try {
      await saveChanges({
        site_name: siteName.trim(), logo_url: logoUrl, accent_color: accent,
        background_color: background, show_powered_by: showPoweredBy,
      });
      setSavedAppearance(appearanceValue);
      setAppearanceNotice("Appearance updated.");
    } catch (cause) { setAppearanceError(cause instanceof Error ? cause.message : "Could not save appearance."); }
    finally { setBusy(false); }
  };

  const togglePublication = async () => {
    if (isPublished && !window.confirm("Unpublish this workspace? Visitors will no longer see its boards, feed, or roadmap until you publish it again.")) return;
    setBusy(true); setPageError(""); setPageNotice("");
    try {
      const updated = await setWorkspacePublication(tenant, readStoredBearerToken(tenant).trim(), !isPublished);
      setIsPublished(updated.is_published);
      setPageNotice(updated.is_published ? "Workspace is live." : "Workspace is now a draft.");
    } catch (cause) { setPageError(cause instanceof Error ? cause.message : "Could not update publication."); }
    finally { setBusy(false); }
  };

  if (loading) return <section className="panel"><p>Loading…</p></section>;
  if (loadError) return <section className="panel"><p className="error-text" role="alert">{loadError}</p></section>;

  return <div className="page-stack" data-unsaved-changes={pagesDirty || appearanceDirty}>
    <section className={`panel workspace-publication${isPublished ? " workspace-publication--live" : ""}`}>
      <div><h2 className="section-title"><span className="publication-dot" data-live={isPublished} aria-hidden="true" />{isPublished ? "Website is live" : "Website is a draft"}</h2>
      <p className="section-subtitle">{isPublished ? "Visitors can see your published boards." : "Publish a board and this workspace to let visitors in."}</p></div>
      <div className="workspace-publication__actions"><a className="ghost-button" href={`${PUBLIC_WEB_URL}/${encodeURIComponent(tenant)}`} target="_blank" rel="noopener noreferrer">Open website ↗</a><button type="button" className={isPublished ? "ghost-button button--danger" : "button button--cta"} disabled={busy || pagesDirty || appearanceDirty} title={pagesDirty || appearanceDirty ? "Save changes first" : undefined} onClick={() => void togglePublication()}>{isPublished ? "Unpublish" : "Publish workspace"}</button></div>
    </section>
    <section className="panel">
      <h2 className="section-title">Visible pages</h2>
      <p className="section-subtitle">Choose what visitors see in this workspace.</p>
      <form className="manage-fields branding-pages" style={{ marginTop: "1rem" }} onSubmit={(event) => void savePages(event)}>
        <label className="manage-check"><input type="checkbox" checked={showBoards} disabled={busy} onChange={(event) => setShowBoards(event.target.checked)} /> <span className="branding-pages__icon"><PageIcon page="boards" /></span><span><strong>Boards</strong><small>Browse discussions, ideas, and reports</small></span></label>
        <label className="manage-check"><input type="checkbox" checked={showFeed} disabled={busy} onChange={(event) => setShowFeed(event.target.checked)} /> <span className="branding-pages__icon"><PageIcon page="feed" /></span><span><strong>Feed</strong><small>Recent activity across this workspace</small></span></label>
        <label className="manage-check"><input type="checkbox" checked={showRoadmap} disabled={busy} onChange={(event) => setShowRoadmap(event.target.checked)} /> <span className="branding-pages__icon"><PageIcon page="roadmap" /></span><span><strong>Roadmap</strong><small>Planned and completed work</small></span></label>
        <h3 className="section-title">Posting policy</h3>
        <label className="manage-check"><input type="checkbox" checked={requirePostApproval} disabled={busy} onChange={(event) => setRequirePostApproval(event.target.checked)} /> Approve every post before publishing</label>
        <p className="section-subtitle">When off, regular posts publish immediately. Suspicious posts still wait for review.</p>
        <div className="manage-row__actions">
          {pagesDirty ? <><span className="section-subtitle">Unsaved changes</span><button className="button button--cta" type="submit" disabled={busy}>{busy ? "Saving…" : "Save settings"}</button></> : null}
        </div>
      </form>
      {pageNotice ? <p className="success-text" role="status">{pageNotice}</p> : null}
      {pageError ? <p className="error-text" role="alert">{pageError}</p> : null}
    </section>

    <section className="panel">
      <h2 className="section-title">Appearance</h2>
      <form className="manage-fields" style={{ marginTop: "1rem" }} onSubmit={(event) => void saveAppearance(event)}>
      <label className="manage-label">Site name
        <input className="manage-input" value={siteName} maxLength={120} onChange={(event) => setSiteName(event.target.value)} required />
      </label>
      <div className="manage-label">Public logo
        <div className="branding-logo-preview" style={{ background }}>
          {logoUrl ? <img src={logoUrl} alt="Current public logo" /> : <strong style={{ color: contrastInk(background) }}>{siteName || "Your workspace"}</strong>}
        </div>
        <input className="manage-input" type="file" accept="image/png,image/jpeg,image/webp" disabled={busy} onChange={(event) => {
          const file = event.target.files?.[0]; event.target.value = ""; void chooseLogo(file);
        }} />
        <span className="section-subtitle">PNG, JPG or WebP · up to 5 MB and 4096 × 4096 pixels. Large images are resized automatically.</span>
        {logoUrl ? <button className="ghost-button" type="button" disabled={busy} onClick={() => { setLogoUrl(null); setAppearanceNotice("Save appearance to remove the logo."); }}>Remove logo</button> : null}
      </div>
      <div className="branding-presets" aria-label="Color themes">{PRESETS.map((preset) => <button
        key={preset.name} type="button" className="branding-preset" disabled={busy}
        onClick={() => { setAccent(preset.accent); setBackground(preset.background); }}
        aria-label={`Use ${preset.name} theme`}
        aria-pressed={accent === preset.accent && background === preset.background}
      ><span style={{ background: preset.accent }} />{preset.name}</button>)}</div>
      <label className="manage-label">Accent color
        <div className="branding-color-field"><input type="color" value={COLOR.test(accent) ? accent : "#e0522f"} onChange={(event) => setAccent(event.target.value)} /><input className="manage-input" value={accent} onChange={(event) => setAccent(event.target.value)} maxLength={7} /></div>
      </label>
      <label className="manage-label">Page background
        <div className="branding-color-field"><input type="color" value={COLOR.test(background) ? background : "#fff3ec"} onChange={(event) => setBackground(event.target.value)} /><input className="manage-input" value={background} onChange={(event) => setBackground(event.target.value)} maxLength={7} /></div>
      </label>
      <label className="manage-check"><input type="checkbox" checked={showPoweredBy} onChange={(event) => setShowPoweredBy(event.target.checked)} /> Show “Powered by Howllo”</label>
      <div className="manage-row__actions">{appearanceDirty ? <><span className="section-subtitle">Unsaved changes</span><button className="button button--cta" type="submit" disabled={busy}>{busy ? "Saving…" : "Save appearance"}</button></> : null}</div>
      </form>
      {appearanceNotice ? <p className="success-text" role="status">{appearanceNotice}</p> : null}
      {appearanceError ? <p className="error-text" role="alert">{appearanceError}</p> : null}
    </section>
  </div>;
}
