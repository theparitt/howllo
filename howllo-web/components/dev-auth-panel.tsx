"use client";

import { useEffect, useState } from "react";
import { DEV_AUTH_COOKIE } from "@/lib/auth-cookie";

const STORAGE_KEY = "howllo.devBearerToken";

export function DevAuthPanel() {
  const [token, setToken] = useState("");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    const existing = window.localStorage.getItem(STORAGE_KEY);
    if (existing) {
      setToken(existing);
      setSaved(true);
    }
  }, []);

  const onSave = () => {
    if (token.trim()) {
      const trimmed = token.trim();
      window.localStorage.setItem(STORAGE_KEY, trimmed);
      document.cookie = `${DEV_AUTH_COOKIE}=${encodeURIComponent(trimmed)}; path=/; max-age=2592000; samesite=lax`;
      setSaved(true);
      return;
    }

    window.localStorage.removeItem(STORAGE_KEY);
    document.cookie = `${DEV_AUTH_COOKIE}=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT`;
    setSaved(false);
  };

  return (
    <div className="auth-panel" style={{ borderRadius: 999, padding: "0.45rem", display: "flex", gap: "0.5rem", alignItems: "center" }}>
      <input
        aria-label="Dev bearer token"
        className="field"
        style={{ minWidth: 260, paddingBlock: "0.7rem" }}
        placeholder="Paste dev Bearer token"
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

export function readStoredBearerToken() {
  if (typeof window === "undefined") return "";
  return window.localStorage.getItem(STORAGE_KEY) ?? "";
}
