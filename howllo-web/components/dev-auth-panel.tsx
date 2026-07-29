"use client";

import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import {
  LEGACY_DEV_AUTH_COOKIE,
  getWorkspaceAuthCookieName,
} from "@/lib/auth-cookie";

const LEGACY_STORAGE_KEY = "howllo.devBearerToken";
const AUTH_EVENT = "howllo-auth-changed";

export function getWorkspaceAuthStorageKey(tenantSlug: string) {
  return `howllo.auth.${tenantSlug.trim()}`;
}

export function getWorkspaceSlugFromPath(pathname: string) {
  const [first] = pathname.replace(/^\/+/, "").split("/");
  if (!first || ["admin", "auth", "dashboard", "boards", "posts", "roadmap", "my", "feed"].includes(first)) {
    return null;
  }
  return first;
}

export function getCurrentWorkspaceSlug() {
  if (typeof window === "undefined") return null;
  return getWorkspaceSlugFromPath(window.location.pathname)
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
