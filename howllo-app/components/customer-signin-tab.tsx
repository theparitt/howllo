"use client";

import { useEffect, useState } from "react";
import { API_BASE_URL } from "@/lib/config";
import { readStoredBearerToken } from "@/components/dev-auth-panel";

type Provider = { key: string; display_name: string; issuer: string; client_id: string; has_client_secret: boolean; token_endpoint_auth_method: string; enabled: boolean; callback_url: string };
type Settings = {
  local_enabled: boolean;
  rooiam_enabled: boolean;
  rooiam_workspace_id: string | null;
  rooiam_client_id: string | null;
  providers: Provider[];
  callback_urls: Record<string, string>;
};
type Choice = "google" | "microsoft" | "oidc";
const choices: { key: Choice; label: string; issuer: string; hint: string }[] = [
  { key: "google", label: "Google", issuer: "https://accounts.google.com", hint: "Google Cloud OAuth web application" },
  { key: "microsoft", label: "Microsoft", issuer: "https://login.microsoftonline.com/{directory-id}/v2.0", hint: "Microsoft Entra tenant-specific application" },
  { key: "oidc", label: "OpenID Connect", issuer: "", hint: "Issuer must be allowed by the Howllo server" },
];

export function CustomerSignInTab({ tenant }: { tenant: string }) {
  const [settings, setSettings] = useState<Settings | null>(null);
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
      .then((value) => { if (active) setSettings(value); })
      .catch((cause) => { if (active) setError(cause.message); });
    return () => { active = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tenant]);

  function select(next: Choice, current = settings?.providers.find((provider) => provider.key === next)) {
    setChoice(next);
    setDisplayName(current?.display_name ?? choices.find((item) => item.key === next)!.label);
    setIssuer(current?.issuer ?? (next === "microsoft" ? "" : choices.find((item) => item.key === next)!.issuer));
    setClientId(current?.client_id ?? "");
    setClientSecret("");
    setAuthMethod(current?.token_endpoint_auth_method ?? "client_secret_post");
    setEnabled(current?.enabled ?? false);
    setError("");
    setNotice("");
  }

  async function saveMethods() {
    if (!settings) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const response = await fetch(api, { method: "PATCH", headers: { "content-type": "application/json", Authorization: token() }, body: JSON.stringify(settings) });
      if (!response.ok) throw new Error(await message(response));
      setSettings(await response.json() as Settings);
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
      select(choice, undefined);
      setNotice("Provider removed.");
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not remove provider."); }
    finally { setBusy(false); }
  }

  if (!settings) return <section className="panel"><p>{error || "Loading customer sign-in…"}</p></section>;
  const existing = settings.providers.find((provider) => provider.key === choice);
  const callbackUrl = settings.callback_urls[choice] ?? existing?.callback_url ?? "";
  return <div className="stack" style={{ maxWidth: 820 }}>
    <section className="panel stack">
      <h2>Sign-in methods</h2>
      <p className="section-subtitle">Choose how customers sign in to this workspace. Staff continue to use their staff account.</p>
      <label><input type="checkbox" checked={settings.local_enabled} onChange={(event) => setSettings({ ...settings, local_enabled: event.target.checked })} /> Password and recovery code</label>
      <label><input type="checkbox" checked={settings.rooiam_enabled} onChange={(event) => setSettings({ ...settings, rooiam_enabled: event.target.checked })} /> RooIAM</label>
      {settings.rooiam_enabled ? <div className="stack">
        <label>RooIAM workspace ID<input className="field" value={settings.rooiam_workspace_id ?? ""} onChange={(event) => setSettings({ ...settings, rooiam_workspace_id: event.target.value })} /></label>
        <label>RooIAM client ID<input className="field" value={settings.rooiam_client_id ?? ""} onChange={(event) => setSettings({ ...settings, rooiam_client_id: event.target.value })} /></label>
        <p className="section-subtitle">Set the redirect URI to your board site’s /auth/callback URL in RooIAM.</p>
      </div> : null}
      <button className="button button--cta" type="button" disabled={busy} onClick={() => void saveMethods()}>Save methods</button>
    </section>
    <section className="panel stack">
      <h2>Social and OpenID Connect</h2>
      <p className="section-subtitle">Use your own app credentials for each provider. Client secrets are stored encrypted and are never shown again.</p>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>{choices.map((item) => <button key={item.key} type="button" className={choice === item.key ? "button button--cta" : "button"} onClick={() => select(item.key)}>{item.label}{settings.providers.some((provider) => provider.key === item.key && provider.enabled) ? " · On" : ""}</button>)}</div>
      <p className="section-subtitle">{choices.find((item) => item.key === choice)?.hint}</p>
      <label>Button label<input className="field" value={displayName} maxLength={60} onChange={(event) => setDisplayName(event.target.value)} /></label>
      <label>Issuer URL<input className="field" value={issuer} placeholder={choices.find((item) => item.key === choice)?.issuer} onChange={(event) => setIssuer(event.target.value)} /></label>
      <label>Client ID<input className="field" value={clientId} onChange={(event) => setClientId(event.target.value)} /></label>
      <label>Client secret<input className="field" type="password" value={clientSecret} autoComplete="new-password" placeholder={existing ? "Leave blank to keep current secret" : "Required"} onChange={(event) => setClientSecret(event.target.value)} /></label>
      {choice === "oidc" ? <label>Token endpoint authentication<select className="field" value={authMethod} onChange={(event) => setAuthMethod(event.target.value)}><option value="client_secret_post">Client secret in form</option><option value="client_secret_basic">Client secret in HTTP Basic</option></select></label> : null}
      <label><input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} /> Show this sign-in option to customers</label>
      <label>Redirect URI<input className="field" readOnly value={callbackUrl} onFocus={(event) => event.currentTarget.select()} /></label>
      <p className="section-subtitle">Add this exact redirect URI to the provider’s app settings before enabling sign-in.</p>
      <div style={{ display: "flex", gap: 8 }}><button className="button button--cta" type="button" disabled={busy} onClick={() => void saveProvider()}>Save provider</button>{existing ? <button className="button" type="button" disabled={busy} onClick={() => void removeProvider()}>Remove</button> : null}</div>
    </section>
    {notice ? <p role="status" className="notice">{notice}</p> : null}
    {error ? <p role="alert" className="notice notice--error">{error}</p> : null}
  </div>;
}

async function message(response: Response): Promise<string> {
  const body = await response.json().catch(() => ({})) as { message?: string; error?: string };
  return body.message || body.error || `Request failed (${response.status}).`;
}
