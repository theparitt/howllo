import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { admin } from "@howllo/api-client";
import { API_BASE_URL } from "@howllo/config";
import type { TenantBranding, WorkspaceAuthConfig } from "@howllo/types";
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

function colorPickerValue(value: string | null | undefined, fallback: string) {
  const trimmed = value?.trim() ?? "";
  return /^#[0-9a-fA-F]{6}$/.test(trimmed) ? trimmed : fallback;
}

function ColorPreviewField({
  label,
  value,
  fallback,
  hint,
  tone,
  onChange,
}: {
  label: string;
  value: string | null | undefined;
  fallback: string;
  hint: string;
  tone: "accent" | "background";
  onChange: (value: string) => void;
}) {
  const resolved = colorPickerValue(value, fallback);
  const style = {
    "--swatch-color": resolved,
  } as CSSProperties;

  return (
    <div className="color-swatch-field">
      <span className="field-label">{label}</span>
      <label className={`color-swatch color-swatch--${tone}`} style={style}>
        <input
          className="color-swatch__input"
          type="color"
          value={resolved}
          onChange={(event) => onChange(event.target.value)}
        />
        <span className="color-swatch__surface">
          <span className="color-swatch__chip" aria-hidden="true" />
          <span className="color-swatch__meta">
            <strong>{resolved}</strong>
            <small>{hint}</small>
          </span>
        </span>
      </label>
    </div>
  );
}

export function SettingsPage() {
  const { client, tenant, authorization } = useSession();
  const api = useMemo(() => admin(client), [client]);
  const [busy, setBusy] = useState<"json" | "csv" | null>(null);
  const [logoUploading, setLogoUploading] = useState(false);
  const logoInputRef = useRef<HTMLInputElement>(null);
  const [brandingBusy, setBrandingBusy] = useState(false);
  const [brandingLoaded, setBrandingLoaded] = useState(false);
  const [authBusy, setAuthBusy] = useState(false);
  const [authLoaded, setAuthLoaded] = useState(false);
  const [brandingMessage, setBrandingMessage] = useState<string | null>(null);
  const [authMessage, setAuthMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [brandingError, setBrandingError] = useState<string | null>(null);
  const [authError, setAuthError] = useState<string | null>(null);
  const [loginProviders, setLoginProviders] = useState<Array<{ id: string; display_name: string; kind: string }>>([]);
  const [providerLoadError, setProviderLoadError] = useState(false);
  const [exportMessage, setExportMessage] = useState<string | null>(null);
  const [branding, setBranding] = useState<TenantBranding>({
    tenant_slug: tenant,
    tenant_name: tenant,
    site_name: tenant,
    logo_url: null,
    accent_color: "#f36949",
    background_color: "#f6e7df",
    show_powered_by: true,
    show_roadmap: true,
  });
  const [workspaceAuth, setWorkspaceAuth] = useState<WorkspaceAuthConfig>({
    tenant_slug: tenant,
    provider: "local",
    rooiam_workspace_id: null,
    rooiam_client_id: null,
    rooiam_widget_base_url: "https://api.rooiam.com/login-widget",
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

  useEffect(() => {
    if (!authorization) {
      setAuthLoaded(false);
      return;
    }

    let cancelled = false;

    async function loadWorkspaceAuth() {
      setAuthBusy(true);
      setAuthError(null);
      setAuthMessage(null);
      try {
        const data = await api.getWorkspaceAuthConfig();
        if (!cancelled) {
          setWorkspaceAuth(data);
          setAuthLoaded(true);
        }
      } catch (e) {
        if (!cancelled) {
          setAuthError(
            e instanceof Error ? e.message : "Failed to load workspace auth configuration",
          );
        }
      } finally {
        if (!cancelled) {
          setAuthBusy(false);
        }
      }
    }

    void loadWorkspaceAuth();
    return () => {
      cancelled = true;
    };
  }, [api, authorization, tenant]);

  useEffect(() => {
    if (!authorization) return;
    let cancelled = false;
    fetch(`${API_BASE_URL}/api/auth/providers`, { cache: "no-store" })
      .then((response) => {
        if (!response.ok) throw new Error("provider list unavailable");
        return response.json() as Promise<Array<{ id: string; display_name: string; kind: string }>>;
      })
      .then((providers) => { if (!cancelled) setLoginProviders(providers); })
      .catch(() => { if (!cancelled) setProviderLoadError(true); });
    return () => { cancelled = true; };
  }, [authorization]);

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
        background_color: branding.background_color?.trim() || undefined,
        show_powered_by: branding.show_powered_by,
        show_roadmap: branding.show_roadmap,
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

  const saveWorkspaceAuth = async () => {
    setAuthBusy(true);
    setAuthError(null);
    setAuthMessage(null);
    try {
      const data = await api.updateWorkspaceAuthConfig({
        provider: workspaceAuth.provider.trim() || "local",
        rooiam_workspace_id: workspaceAuth.rooiam_workspace_id?.trim() || "",
        rooiam_client_id: workspaceAuth.rooiam_client_id?.trim() || "",
        rooiam_widget_base_url: workspaceAuth.rooiam_widget_base_url?.trim() || "",
      });
      setWorkspaceAuth(data);
      setAuthLoaded(true);
      setAuthMessage("Workspace auth updated.");
    } catch (e) {
      setAuthError(
        e instanceof Error ? e.message : "Failed to save workspace auth configuration",
      );
    } finally {
      setAuthBusy(false);
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
            <ColorPreviewField
              label="Accent preview"
              value={branding.accent_color}
              fallback="#f36949"
              hint="Click to choose the accent swatch"
              tone="accent"
              onChange={(value) =>
                setBranding((current) => ({
                  ...current,
                  accent_color: value,
                }))
              }
            />
            <label>
              Background color
              <input
                value={branding.background_color ?? ""}
                onChange={(event) =>
                  setBranding((current) => ({
                    ...current,
                    background_color: event.target.value,
                  }))
                }
                placeholder="#f6e7df"
              />
            </label>
            <ColorPreviewField
              label="Background preview"
              value={branding.background_color}
              fallback="#f6e7df"
              hint="Click to choose the workspace backdrop"
              tone="background"
              onChange={(value) =>
                setBranding((current) => ({
                  ...current,
                  background_color: value,
                }))
              }
            />
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
            <label className="checkbox field-span-2">
              <input
                type="checkbox"
                checked={branding.show_roadmap}
                onChange={(event) =>
                  setBranding((current) => ({
                    ...current,
                    show_roadmap: event.target.checked,
                  }))
                }
              />
              Show roadmap in the public workspace navigation
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

        <Panel title="Workspace Auth">
          <p className="muted">
            Howllo loads enabled login providers from the server. Configure local
            accounts and OIDC in the server environment. RooIAM widget settings
            below are optional for existing workspace login widgets.
          </p>
          {providerLoadError ? <p className="muted">Could not load enabled login providers.</p> :
            <p className="muted">Enabled: {[...loginProviders.map((provider) => provider.display_name), ...(workspaceAuth.provider === "rooiam" ? ["RooIAM widget"] : [])].join(", ") || "None"}</p>}
          <div className="form-grid">
            <label className="field-span-2">
              Legacy workspace widget
              <select
                value={workspaceAuth.provider}
                onChange={(event) =>
                  setWorkspaceAuth((current) => ({
                    ...current,
                    provider: event.target.value,
                  }))
                }
              >
                {loginProviders.some((provider) => provider.id === "local") ? <option value="local">None</option> : null}
                <option value="rooiam">RooIAM widget</option>
              </select>
            </label>
            {workspaceAuth.provider === "rooiam" ? <>
            <label className="field-span-2">
              RooIAM workspace ID
              <input
                value={workspaceAuth.rooiam_workspace_id ?? ""}
                onChange={(event) =>
                  setWorkspaceAuth((current) => ({
                    ...current,
                    rooiam_workspace_id: event.target.value || null,
                  }))
                }
                placeholder="59d94bdf-2549-4fb8-b57c-900a6a67f3dd"
              />
            </label>
            <label className="field-span-2">
              RooIAM client ID
              <input
                value={workspaceAuth.rooiam_client_id ?? ""}
                onChange={(event) =>
                  setWorkspaceAuth((current) => ({
                    ...current,
                    rooiam_client_id: event.target.value || null,
                  }))
                }
                placeholder="client_id for this workspace"
              />
            </label>
            <label className="field-span-2">
              RooIAM widget base URL
              <input
                value={workspaceAuth.rooiam_widget_base_url ?? ""}
                onChange={(event) =>
                  setWorkspaceAuth((current) => ({
                    ...current,
                    rooiam_widget_base_url: event.target.value || null,
                  }))
                }
                placeholder="https://api.rooiam.com/login-widget"
              />
            </label>
            </> : null}
          </div>
          <div className="button-row" style={{ marginTop: 16 }}>
            <button
              className="primary"
              disabled={authBusy}
              onClick={saveWorkspaceAuth}
            >
              {authBusy ? "Saving..." : "Save workspace auth"}
            </button>
            {authLoaded ? (
              <span className="badge">
                Current provider: {workspaceAuth.provider || "unset"}
              </span>
            ) : null}
          </div>
          {authMessage ? <p className="muted">{authMessage}</p> : null}
          {authError ? <p className="error-text">{authError}</p> : null}
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
