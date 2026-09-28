"use client";

import { useEffect, useState } from "react";
import { API_BASE_URL } from "@/lib/config";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import { ProviderMark } from "@/components/provider-mark";

type Provider = { key: string; display_name: string; issuer: string; client_id: string; has_client_secret: boolean; token_endpoint_auth_method: string; enabled: boolean; callback_url: string };
type Settings = {
  local_enabled: boolean;
  rooiam_enabled: boolean;
  rooiam_workspace_id: string | null;
  rooiam_client_id: string | null;
  providers: Provider[];
  callback_urls: Record<string, string>;
  callback_url_configured: boolean;
};
type Choice = "google" | "microsoft" | "oidc";
const choices: { key: Choice; label: string; issuer: string; hint: string }[] = [
  { key: "google", label: "Google", issuer: "https://accounts.google.com", hint: "Google Cloud OAuth web application" },
  { key: "microsoft", label: "Microsoft", issuer: "https://login.microsoftonline.com/{directory-id}/v2.0", hint: "Microsoft Entra tenant-specific application" },
  { key: "oidc", label: "OpenID Connect", issuer: "", hint: "Issuer must be allowed by the Howllo server" },
];

export function CustomerSignInTab({ tenant }: { tenant: string }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [savedMethods, setSavedMethods] = useState("");
  const [choice, setChoice] = useState<Choice>("google");
  const [displayName, setDisplayName] = useState("Google");
  const [issuer, setIssuer] = useState("https://accounts.google.com");
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [authMethod, setAuthMethod] = useState("client_secret_post");
  const [enabled, setEnabled] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const api = `${API_BASE_URL}/api/tenants/${encodeURIComponent(tenant)}/customer-auth`;
  const token = () => readStoredBearerToken(tenant).trim();

  useEffect(() => {
    let active = true;
    fetch(api, { headers: { Authorization: token() }, cache: "no-store" })
      .then(async (response) => { if (!response.ok) throw new Error("Could not load sign-in settings."); return response.json() as Promise<Settings>; })
      .then((value) => { if (active) { setSettings(value); setSavedMethods(JSON.stringify([value.local_enabled, value.rooiam_enabled, value.rooiam_workspace_id, value.rooiam_client_id])); const initial = value.providers.find((provider) => provider.key === "google"); if (initial) { setDisplayName(initial.display_name); setIssuer(initial.issuer); setClientId(initial.client_id); setAuthMethod(initial.token_endpoint_auth_method); setEnabled(initial.enabled); } } })
      .catch((cause) => { if (active) setError(cause.message); });
    return () => { active = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tenant]);

  function select(next: Choice, current?: Provider | null) {
    const provider = current === undefined ? settings?.providers.find((item) => item.key === next) : current;
    setChoice(next);
    setDisplayName(provider?.display_name ?? choices.find((item) => item.key === next)!.label);
    setIssuer(provider?.issuer ?? (next === "microsoft" ? "" : choices.find((item) => item.key === next)!.issuer));
    setClientId(provider?.client_id ?? "");
    setClientSecret("");
    setAuthMethod(provider?.token_endpoint_auth_method ?? "client_secret_post");
    setEnabled(provider?.enabled ?? false);
    setError("");
    setNotice("");
  }

  async function saveMethods() {
    if (!settings) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const response = await fetch(api, { method: "PATCH", headers: { "content-type": "application/json", Authorization: token() }, body: JSON.stringify(settings) });
      if (!response.ok) throw new Error(await message(response));
      const updated = await response.json() as Settings;
      setSettings(updated);
      setSavedMethods(JSON.stringify([updated.local_enabled, updated.rooiam_enabled, updated.rooiam_workspace_id, updated.rooiam_client_id]));
      setNotice("Customer sign-in methods saved.");
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not save settings."); }
    finally { setBusy(false); }
  }

  async function saveProvider() {
    setBusy(true); setError(""); setNotice("");
    try {
      const response = await fetch(`${api}/providers/${choice}`, {
        method: "PUT",
        headers: { "content-type": "application/json", Authorization: token() },
        body: JSON.stringify({ display_name: displayName, issuer, client_id: clientId, client_secret: clientSecret || undefined, token_endpoint_auth_method: authMethod, enabled }),
      });
      if (!response.ok) throw new Error(await message(response));
      setSettings(await response.json() as Settings);
      setClientSecret("");
      setNotice(`${displayName} saved.`);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not save provider."); }
    finally { setBusy(false); }
  }

  async function removeProvider() {
    if (!window.confirm(`Remove ${displayName} sign-in from this workspace?`)) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const response = await fetch(`${api}/providers/${choice}`, { method: "DELETE", headers: { Authorization: token() } });
      if (!response.ok) throw new Error(await message(response));
      const updated: Settings = { ...settings!, providers: settings!.providers.filter((provider) => provider.key !== choice) };
      setSettings(updated);
      select(choice, null);
      setNotice("Provider removed.");
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not remove provider."); }
    finally { setBusy(false); }
  }

  const methodsDirty = Boolean(settings) && savedMethods !== JSON.stringify([settings?.local_enabled, settings?.rooiam_enabled, settings?.rooiam_workspace_id, settings?.rooiam_client_id]);
  const existingProvider = settings?.providers.find((provider) => provider.key === choice);
  const preset = choices.find((item) => item.key === choice)!;
  const providerDirty = Boolean(settings) && JSON.stringify([displayName, issuer, clientId, authMethod, enabled, clientSecret]) !== JSON.stringify([existingProvider?.display_name ?? preset.label, existingProvider?.issuer ?? (choice === "microsoft" ? "" : preset.issuer), existingProvider?.client_id ?? "", existingProvider?.token_endpoint_auth_method ?? "client_secret_post", existingProvider?.enabled ?? false, ""]);
  useEffect(() => {
    if (!methodsDirty && !providerDirty) return;
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [methodsDirty, providerDirty]);

  if (!settings) return <section className="panel"><p>{error || "Loading customer sign-in…"}</p></section>;
  const existing = settings.providers.find((provider) => provider.key === choice);
  const callbackUrl = settings.callback_urls[choice] ?? existing?.callback_url ?? "";
  return <div className="stack" style={{ maxWidth: 820 }} data-unsaved-changes={methodsDirty || providerDirty}>
    <section className="panel stack">
      <h2>Methods</h2>
      <p className="section-subtitle">Choose how customers sign in to this workspace. Staff continue to use their staff account.</p>
      <label><input type="checkbox" checked={settings.local_enabled} onChange={(event) => setSettings({ ...settings, local_enabled: event.target.checked })} /> Password and recovery code</label>
      <label><input type="checkbox" checked={settings.rooiam_enabled} onChange={(event) => setSettings({ ...settings, rooiam_enabled: event.target.checked })} /> RooIAM</label>
      {settings.rooiam_enabled ? <div className="stack">
        <label>RooIAM workspace ID<input className="field" value={settings.rooiam_workspace_id ?? ""} onChange={(event) => setSettings({ ...settings, rooiam_workspace_id: event.target.value })} /></label>
        <label>RooIAM client ID<input className="field" value={settings.rooiam_client_id ?? ""} onChange={(event) => setSettings({ ...settings, rooiam_client_id: event.target.value })} /></label>
        <p className="section-subtitle">Set the redirect URI to your website’s /auth/callback URL in RooIAM.</p>
      </div> : null}
      {methodsDirty ? <div className="manage-row__actions"><span className="section-subtitle">Unsaved changes</span><button className="button button--cta" type="button" disabled={busy} onClick={() => void saveMethods()}>{busy ? "Saving…" : "Save methods"}</button></div> : null}
    </section>
    <section className="panel stack">
      <h2>Social and OpenID Connect</h2>
      <p className="section-subtitle">Use your own app credentials for each provider. Client secrets are stored encrypted and are never shown again.</p>
      {!settings.callback_url_configured ? <p className="notice" role="status">Social sign-in needs a public API URL. Ask the platform administrator to configure it before adding a provider.</p> : null}
      <div className="provider-tabs" role="group" aria-label="Sign-in provider">{choices.map((item) => <button key={item.key} type="button" className="provider-choice" data-provider={item.key} aria-pressed={choice === item.key} onClick={() => { if (item.key !== choice && (!providerDirty || window.confirm("You have unsaved provider changes. Discard them?"))) select(item.key); }}><span className="provider-choice__mark"><ProviderMark provider={item.key} /></span><span>{item.label}</span>{settings.providers.some((provider) => provider.key === item.key && provider.enabled) ? <span className="provider-choice__status">On</span> : null}</button>)}</div>
      <p className="section-subtitle">{choices.find((item) => item.key === choice)?.hint}</p>
      <label>Button label<input className="field" value={displayName} maxLength={60} onChange={(event) => setDisplayName(event.target.value)} /></label>
      <label>Issuer URL<input className="field" value={issuer} placeholder={choices.find((item) => item.key === choice)?.issuer} onChange={(event) => setIssuer(event.target.value)} /></label>
      <label>Client ID<input className="field" value={clientId} onChange={(event) => setClientId(event.target.value)} /></label>
      <label>Client secret<input className="field" type="password" value={clientSecret} autoComplete="new-password" placeholder={existing ? "Leave blank to keep current secret" : "Required"} onChange={(event) => setClientSecret(event.target.value)} /></label>
      {choice === "oidc" ? <label>Token endpoint authentication<select className="field" value={authMethod} onChange={(event) => setAuthMethod(event.target.value)}><option value="client_secret_post">Client secret in form</option><option value="client_secret_basic">Client secret in HTTP Basic</option></select></label> : null}
      <label><input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} /> Show this sign-in option to customers</label>
      <label>Redirect URI<input className="field" readOnly value={callbackUrl} placeholder={settings.callback_url_configured ? undefined : "Available after platform setup"} onFocus={(event) => event.currentTarget.select()} /></label>
      {settings.callback_url_configured ? <p className="section-subtitle">Add this exact redirect URI to the provider’s app settings before enabling sign-in.</p> : null}
      <div className="manage-row__actions" style={{ flexWrap: "wrap" }}>{providerDirty ? <><span className="section-subtitle">Unsaved changes</span><button className="button button--cta" type="button" disabled={busy || !settings.callback_url_configured || !clientId.trim() || (!existing && !clientSecret.trim())} onClick={() => void saveProvider()}>{busy ? "Saving…" : "Save provider"}</button></> : null}{existing ? <button className="ghost-button button--danger" type="button" disabled={busy} onClick={() => void removeProvider()}>Remove provider</button> : null}</div>
    </section>
    {notice ? <p role="status" className="notice">{notice}</p> : null}
    {error ? <p role="alert" className="notice notice--error">{error}</p> : null}
  </div>;
}

async function message(response: Response): Promise<string> {
  const body = await response.json().catch(() => ({})) as { message?: string; error?: string };
  return body.message || body.error || `Request failed (${response.status}).`;
}
