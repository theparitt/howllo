"use client";

import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import {
  LEGACY_DEV_AUTH_COOKIE,
  getWorkspaceAuthCookieName,
} from "@/lib/auth-cookie";
import { subdomainSlug } from "@/lib/subdomain";

const LEGACY_STORAGE_KEY = "howllo.devBearerToken";
const AUTH_EVENT = "howllo-auth-changed";

export function getWorkspaceAuthStorageKey(tenantSlug: string) {
  return `howllo.auth.${tenantSlug.trim()}`;
}

export function getWorkspaceSlugFromPath(pathname: string) {
  const [first, second] = pathname.replace(/^\/+/, "").split("/");
  if (first === "app") return second || null;
  if (!first || ["admin", "auth", "dashboard", "boards", "posts", "roadmap", "my", "feed", "manage"].includes(first)) {
    return null;
  }
  return first;
}

export function getCurrentWorkspaceSlug() {
  if (typeof window === "undefined") return null;
  // On a workspace subdomain the slug is in the host, not the path.
  return subdomainSlug(window.location.host)
    ?? getWorkspaceSlugFromPath(window.location.pathname)
    ?? new URLSearchParams(window.location.search).get("tenant")?.trim()
    ?? null;
}

function writeAuthCookie(tenantSlug: string, value: string) {
  const cookieName = getWorkspaceAuthCookieName(tenantSlug);
  if (value) {
    document.cookie = `${cookieName}=${encodeURIComponent(value)}; path=/; max-age=2592000; samesite=lax`;
  } else {
    document.cookie = `${cookieName}=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT`;
  }
}

function notifyAuthChanged() {
  window.dispatchEvent(new Event(AUTH_EVENT));
}

export function persistBearerToken(tenantSlug: string, value: string) {
  if (typeof window === "undefined") return;
  const trimmed = value.trim();
  const workspace = tenantSlug.trim();
  if (!workspace) return;
  if (!trimmed) {
    clearStoredBearerToken(workspace);
    return;
  }
  window.localStorage.setItem(getWorkspaceAuthStorageKey(workspace), trimmed);
  window.localStorage.removeItem(LEGACY_STORAGE_KEY);
  document.cookie = `${LEGACY_DEV_AUTH_COOKIE}=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT`;
  writeAuthCookie(workspace, trimmed);
  notifyAuthChanged();
}

export function clearStoredBearerToken(tenantSlug: string) {
  if (typeof window === "undefined") return;
  const workspace = tenantSlug.trim();
  if (!workspace) return;
  window.localStorage.removeItem(getWorkspaceAuthStorageKey(workspace));
  writeAuthCookie(workspace, "");
  notifyAuthChanged();
}

export function subscribeToBearerTokenChange(listener: () => void) {
  if (typeof window === "undefined") return () => {};
  window.addEventListener(AUTH_EVENT, listener);
  window.addEventListener("storage", listener);
  return () => {
    window.removeEventListener(AUTH_EVENT, listener);
    window.removeEventListener("storage", listener);
  };
}

export function DevAuthPanel() {
  const pathname = usePathname();
  const workspace = getWorkspaceSlugFromPath(pathname)
    ?? (typeof window !== "undefined"
      ? new URLSearchParams(window.location.search).get("tenant")?.trim() || null
      : null);
  const [token, setToken] = useState("");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    const existing = workspace
      ? window.localStorage.getItem(getWorkspaceAuthStorageKey(workspace))
      : "";
    if (existing) {
      setToken(existing);
      setSaved(true);
    }
  }, [workspace]);

  const onSave = () => {
    if (!workspace) {
      setSaved(false);
      return;
    }
    if (token.trim()) {
      persistBearerToken(workspace, token);
      setSaved(true);
      return;
    }

    clearStoredBearerToken(workspace);
    setSaved(false);
  };

  return (
    <div className="auth-panel" style={{ borderRadius: 999, padding: "0.45rem", display: "flex", gap: "0.5rem", alignItems: "center" }}>
      <input
        aria-label="Workspace session token"
        className="field"
        style={{ minWidth: 260, paddingBlock: "0.7rem" }}
        placeholder={workspace ? `Paste ${workspace} session token` : "Open a workspace first"}
        value={token}
        onChange={(event) => setToken(event.target.value)}
      />
      <button className="button" onClick={onSave} type="button">
        Save
      </button>
      {saved ? <span className="muted" style={{ fontSize: "0.82rem" }}>ready</span> : null}
    </div>
  );
}

export function readStoredBearerToken(tenantSlug: string) {
  if (typeof window === "undefined") return "";
  const workspace = tenantSlug.trim();
  if (!workspace) return "";
  return window.localStorage.getItem(getWorkspaceAuthStorageKey(workspace)) ?? "";
}

// Account-level credential for signing into the tenant dashboard before any
// workspace exists. Local and OIDC callbacks both set it.
const ACCOUNT_TOKEN_KEY = "howllo.account";

export function writeAccountToken(value: string) {
  if (typeof window === "undefined") return;
  if (value.trim()) {
    window.localStorage.setItem(ACCOUNT_TOKEN_KEY, value.trim());
  } else {
    window.localStorage.removeItem(ACCOUNT_TOKEN_KEY);
  }
  notifyAuthChanged();
}

export function readAccountToken(): string {
  if (typeof window === "undefined") return "";
  return window.localStorage.getItem(ACCOUNT_TOKEN_KEY) ?? "";
}

export function clearAllStoredBearerTokens() {
  if (typeof window === "undefined") return;
  const workspaces: string[] = [];
  for (let i = 0; i < window.localStorage.length; i += 1) {
    const key = window.localStorage.key(i);
    if (key?.startsWith("howllo.auth.")) {
      workspaces.push(key.slice("howllo.auth.".length));
    }
  }
  for (const workspace of workspaces) clearStoredBearerToken(workspace);
  window.localStorage.removeItem(LEGACY_STORAGE_KEY);
  document.cookie = `${LEGACY_DEV_AUTH_COOKIE}=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT`;
  writeAccountToken("");
}

// The best available account credential: an explicit account token, else any
// stored workspace-session token (the API scopes by the resolved user either way).
export function readAnyStoredBearerToken(): string {
  if (typeof window === "undefined") return "";
  const account = readAccountToken().trim();
  if (account) return account;
  for (let i = 0; i < window.localStorage.length; i += 1) {
    const key = window.localStorage.key(i);
    if (key && key.startsWith("howllo.auth.")) {
      const value = window.localStorage.getItem(key);
      if (value && value.trim()) return value;
    }
  }
  return "";
}
