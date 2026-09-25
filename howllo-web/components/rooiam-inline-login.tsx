"use client";

import { useEffect, useRef, useState } from "react";
import { getWorkspaceAuthConfig } from "@/lib/api";
import { rememberRooiamReturnTo } from "@/lib/rooiam-auth";

const base = process.env.NEXT_PUBLIC_ROOIAM_WIDGET_BASE_URL?.trim();
const workspaceId = process.env.NEXT_PUBLIC_ROOIAM_WIDGET_WORKSPACE_ID?.trim();
const clientId = process.env.NEXT_PUBLIC_ROOIAM_WIDGET_CLIENT_ID?.trim();

function widgetUrl(widgetBase: string | null | undefined, widgetWorkspaceId: string | null | undefined, widgetClientId: string | null | undefined) {
  if (!widgetBase || !widgetWorkspaceId || !widgetClientId) return "";
  try {
    const url = new URL(widgetBase);
    url.searchParams.set("workspace_id", widgetWorkspaceId);
    url.searchParams.set("client_id", widgetClientId);
    return url.toString();
  } catch {
    return "";
  }
}

export function RooiamInlineLogin({ tenantSlug }: { tenantSlug?: string }) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(520);
  const [url, setUrl] = useState(() => tenantSlug ? "" : widgetUrl(base, workspaceId, clientId));
  const [loading, setLoading] = useState(Boolean(tenantSlug));

  useEffect(() => {
    if (!tenantSlug) return;
    let cancelled = false;
    setLoading(true);
    getWorkspaceAuthConfig(tenantSlug)
      .then((config) => {
        if (!cancelled) {
          setUrl(config.provider === "rooiam" ? widgetUrl(config.rooiam_widget_base_url, config.rooiam_workspace_id, config.rooiam_client_id) : "");
          setLoading(false);
        }
      })
      .catch(() => { if (!cancelled) { setUrl(""); setLoading(false); } });
    return () => { cancelled = true; };
  }, [tenantSlug]);

  useEffect(() => {
    if (!url) return;
    const origin = new URL(url).origin;
    function onMessage(event: MessageEvent) {
      if (event.origin !== origin || event.source !== frame.current?.contentWindow) return;
      if (event.data?.type === "rooiam-login-widget:size") {
        const nextHeight = Number(event.data.height);
        if (Number.isFinite(nextHeight) && nextHeight >= 300 && nextHeight <= 2000) {
          setHeight(Math.max(420, Math.min(680, Math.ceil(nextHeight))));
        }
        return;
      }
      if (event.data?.type === "rooiam:navigate" && typeof event.data.url === "string") {
        rememberRooiamReturnTo(`${window.location.pathname}${window.location.search}`);
        window.location.href = event.data.url;
      }
    }
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [url]);

  if (loading) return <p className="section-subtitle">Loading workspace sign-in…</p>;
  if (!url) {
    return <p className="notice notice--error" role="alert">RooIAM sign-in is not configured. Contact your administrator.</p>;
  }

  return (
    <iframe
      ref={frame}
      title="Sign in with RooIAM"
      src={url}
      allow="publickey-credentials-get *"
      loading="eager"
      className="rooiam-inline-login__frame"
      style={{ height }}
    />
  );
}
