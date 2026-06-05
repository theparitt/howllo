import { useEffect, useMemo, useRef, useState } from "react";
import { admin } from "@howllo/api-client";
import { BOARD_WEB_BASE_URL, publicRoutes } from "@howllo/config";
import type { TenantBranding } from "@howllo/types";
import { Panel } from "../../components/Panel";
import { SessionRequired } from "../../components/SessionRequired";
import { useSession } from "../../lib/session";

function downloadFile(filename: string, content: string, type: string) {
  const blob = new Blob([content], { type });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

export function SettingsPage() {
  const { client, tenant, authorization } = useSession();
  const api = useMemo(() => admin(client), [client]);
  const [busy, setBusy] = useState<"json" | "csv" | null>(null);
  const [logoUploading, setLogoUploading] = useState(false);
  const logoInputRef = useRef<HTMLInputElement>(null);
  const [brandingBusy, setBrandingBusy] = useState(false);
  const [brandingLoaded, setBrandingLoaded] = useState(false);
  const [brandingMessage, setBrandingMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [brandingError, setBrandingError] = useState<string | null>(null);
  const [exportMessage, setExportMessage] = useState<string | null>(null);
  const [branding, setBranding] = useState<TenantBranding>({
    tenant_slug: tenant,
    tenant_name: tenant,
    site_name: tenant,
    logo_url: null,
    accent_color: "#f36949",
    show_powered_by: true,
  });

  useEffect(() => {
    if (!authorization) {
      setBrandingLoaded(false);
      return;
    }

    let cancelled = false;

    async function loadBranding() {
      setBrandingBusy(true);
      setBrandingError(null);
      setBrandingMessage(null);
      try {
        const data = await api.getTenantBranding();
        if (!cancelled) {
          setBranding(data);
          setBrandingLoaded(true);
        }
      } catch (e) {
        if (!cancelled) {
          setBrandingError(
            e instanceof Error ? e.message : "Failed to load workspace branding",
          );
        }
      } finally {
        if (!cancelled) {
          setBrandingBusy(false);
        }
      }
    }

    void loadBranding();
    return () => {
      cancelled = true;
    };
  }, [api, authorization, tenant]);

  if (!authorization) {
    return (
      <section>
        <h1>Workspace Settings</h1>
        <SessionRequired />
      </section>
    );
  }

  const exportJson = async () => {
    setBusy("json");
    setError(null);
    setExportMessage(null);
    try {
      const data = await api.exportPostsJson();
      downloadFile(
        `${tenant}-posts.json`,
        JSON.stringify(data, null, 2),
        "application/json",
      );
      setExportMessage("JSON export started.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to export JSON");
    } finally {
      setBusy(null);
    }
  };

  const exportCsv = async () => {
    setBusy("csv");
    setError(null);
    setExportMessage(null);
    try {
      const csv = await api.exportPostsCsv();
      downloadFile(`${tenant}-posts.csv`, csv, "text/csv;charset=utf-8");
      setExportMessage("CSV export started.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to export CSV");
    } finally {
      setBusy(null);
    }
  };

  const onPickLogo = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    if (!file.type.startsWith("image/")) {
      setBrandingError("Please choose an image file.");
      return;
    }
    setLogoUploading(true);
    setBrandingError(null);
    setBrandingMessage(null);
    try {
      const url = await client.uploadImage(file);
      setBranding((current) => ({ ...current, logo_url: url }));
      setBrandingMessage("Logo uploaded. Save branding to apply it.");
    } catch (e) {
      setBrandingError(e instanceof Error ? e.message : "Logo upload failed.");
    } finally {
      setLogoUploading(false);
    }
  };

  const saveBranding = async () => {
    setBrandingBusy(true);
    setBrandingError(null);
    setBrandingMessage(null);
    try {
      const data = await api.updateTenantBranding({
        site_name: branding.site_name.trim(),
        logo_url: branding.logo_url?.trim() || undefined,
        accent_color: branding.accent_color?.trim() || undefined,
        show_powered_by: branding.show_powered_by,
      });
      setBranding(data);
      setBrandingLoaded(true);
      setBrandingMessage("Branding updated.");
    } catch (e) {
      setBrandingError(
        e instanceof Error ? e.message : "Failed to save workspace branding",
      );
    } finally {
      setBrandingBusy(false);
    }
  };

  return (
    <section>
      <h1>Workspace Settings</h1>
      <p className="muted">
        Workspace-level controls and export operations for <strong>{tenant}</strong>.
      </p>

      <div className="content-grid">
        <Panel title="Branding">
          <p className="muted">
            Make the public feedback board look like your product, while keeping
            Howllo as the underlying system.
          </p>
          <div className="form-grid">
            <label className="field-span-2">
              Site name
              <input
                value={branding.site_name}
                onChange={(event) =>
                  setBranding((current) => ({
                    ...current,
                    site_name: event.target.value,
                  }))
                }
                placeholder="Howllo Feedback"
              />
            </label>
            <div className="field-span-2">
              <span className="field-label">Workspace logo</span>
              <div className="logo-uploader">
                <div className="logo-preview">
                  {branding.logo_url ? (
                    <img src={branding.logo_url} alt="Workspace logo" />
                  ) : (
                    <span className="logo-preview__empty">No logo</span>
                  )}
                </div>
                <div className="logo-uploader__actions">
                  <input
                    ref={logoInputRef}
                    type="file"
                    accept="image/*"
                    hidden
                    onChange={onPickLogo}
                  />
                  <button
                    type="button"
                    disabled={logoUploading}
                    onClick={() => logoInputRef.current?.click()}
                  >
                    {logoUploading ? "Uploading..." : branding.logo_url ? "Replace" : "Upload logo"}
                  </button>
                  {branding.logo_url ? (
                    <button
                      type="button"
                      className="danger"
                      onClick={() =>
                        setBranding((current) => ({ ...current, logo_url: null }))
                      }
                    >
                      Remove
                    </button>
                  ) : null}
                  <p className="muted small">PNG, JPG, or SVG. Stored in your configured storage.</p>
                </div>
              </div>
              <input
                value={branding.logo_url ?? ""}
                onChange={(event) =>
                  setBranding((current) => ({
                    ...current,
                    logo_url: event.target.value || null,
                  }))
                }
                placeholder="...or paste an image URL"
                style={{ marginTop: 10 }}
              />
            </div>
            <label>
              Accent color
              <input
                value={branding.accent_color ?? ""}
                onChange={(event) =>
                  setBranding((current) => ({
                    ...current,
                    accent_color: event.target.value,
                  }))
                }
                placeholder="#f36949"
              />
            </label>
            <label>
              Accent preview
              <input
                type="color"
                value={branding.accent_color ?? "#f36949"}
                onChange={(event) =>
                  setBranding((current) => ({
                    ...current,
                    accent_color: event.target.value,
                  }))
                }
              />
            </label>
            <label className="checkbox field-span-2">
              <input
                type="checkbox"
                checked={branding.show_powered_by}
                onChange={(event) =>
                  setBranding((current) => ({
                    ...current,
                    show_powered_by: event.target.checked,
                  }))
                }
              />
              Show "Powered by Howllo" in the public footer
            </label>
          </div>
          <div className="button-row" style={{ marginTop: 16 }}>
            <button
              className="primary"
              disabled={brandingBusy}
              onClick={saveBranding}
            >
              {brandingBusy ? "Saving..." : "Save branding"}
            </button>
              {brandingLoaded ? (
                <span className="badge">
                  Preview workspace: {branding.site_name || branding.tenant_name}
                </span>
              ) : null}
          </div>
          {brandingMessage ? <p className="muted">{brandingMessage}</p> : null}
          {brandingError ? <p className="error-text">{brandingError}</p> : null}
        </Panel>

        <Panel title="Active workspace">
          <div className="button-row" style={{ marginBottom: 16 }}>
            <a
              href={`${BOARD_WEB_BASE_URL}${publicRoutes.roadmap(tenant)}`}
              target="_blank"
              rel="noreferrer"
            >
              Open public roadmap
            </a>
            <a
              href={`${BOARD_WEB_BASE_URL}${publicRoutes.workspace(tenant)}`}
              target="_blank"
              rel="noreferrer"
            >
              Open public workspace
            </a>
          </div>
          <dl className="detail-list">
            <div>
              <dt>Workspace slug</dt>
              <dd>
                <code>{tenant}</code>
              </dd>
            </div>
            <div>
              <dt>Session</dt>
              <dd>{authorization ? "Signed in" : "Not signed in"}</dd>
            </div>
            <div>
              <dt>Control model</dt>
              <dd>Workspace management surface, separate from board moderation.</dd>
            </div>
            <div>
              <dt>Public site name</dt>
              <dd>{branding.site_name || branding.tenant_name}</dd>
            </div>
          </dl>
        </Panel>

        <Panel title="Exports">
          <p className="muted">
            Export current workspace posts for backups, analysis, or migration.
          </p>
          <div className="button-row">
            <button className="primary" disabled={busy !== null} onClick={exportJson}>
              {busy === "json" ? "Exporting JSON..." : "Download JSON"}
            </button>
            <button disabled={busy !== null} onClick={exportCsv}>
              {busy === "csv" ? "Exporting CSV..." : "Download CSV"}
            </button>
          </div>
          {exportMessage ? <p className="muted">{exportMessage}</p> : null}
          {error ? <p className="error-text">{error}</p> : null}
        </Panel>
      </div>
    </section>
  );
}
