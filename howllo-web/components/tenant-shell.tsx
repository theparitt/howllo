"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { type CSSProperties, useEffect, useMemo, useState } from "react";
import { AuthControl } from "@/components/auth-control";
import { NotificationBell } from "@/components/notification-bell";
import { getMyWorkspaceRole, getTenantBranding } from "@/lib/api";
import { readAnyStoredBearerToken, readStoredBearerToken, subscribeToBearerTokenChange } from "@/components/dev-auth-panel";
import { ENABLED_AUTH_PROVIDERS } from "@/lib/auth-provider";
import type { TenantBranding } from "@/lib/types";
import { buildTenantPath } from "@/lib/default-tenant";
import { subdomainSlug } from "@/lib/subdomain";
import { contrastInk, hexToRgba } from "@/lib/theme";

const RESERVED_TOP_LEVEL_ROUTES = new Set([
  "",
  "dashboard",
  "boards",
  "posts",
  "roadmap",
  "admin",
  "my",
  "feed",
  "manage",
  "app",
]);

const DEFAULT_BRANDING: TenantBranding = {
  tenant_slug: "",
  tenant_name: "Howllo",
  site_name: "Howllo",
  logo_url: null,
  accent_color: null,
  background_color: null,
  show_powered_by: true,
  show_roadmap: true,
  show_boards: true,
  show_feed: true,
};

function getTenantSlugFromPath(pathname: string): string | null {
  const [first, second] = pathname.replace(/^\/+/, "").split("/");
  if (first === "app") return second || null;
  if (RESERVED_TOP_LEVEL_ROUTES.has(first ?? "")) {
    return null;
  }
  return first || null;
}

export function TenantShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const [branding, setBranding] = useState<TenantBranding>(DEFAULT_BRANDING);
  const [canManage, setCanManage] = useState(false);
  const [hasSession, setHasSession] = useState(false);

  useEffect(() => {
    const sync = () => setHasSession(Boolean(readAnyStoredBearerToken().trim()));
    sync();
    return subscribeToBearerTokenChange(sync);
  }, []);

  // Show the Manage link only to workspace owners/admins.
  useEffect(() => {
    const check = () => {
      const slug =
        (typeof window !== "undefined" ? subdomainSlug(window.location.host) : null) ??
        getTenantSlugFromPath(pathname) ??
        (typeof window !== "undefined"
          ? new URLSearchParams(window.location.search).get("tenant")?.trim() || null
          : null);
      const token = slug ? readStoredBearerToken(slug).trim() : "";
      if (!slug || !token) {
        setCanManage(false);
        return;
      }
      getMyWorkspaceRole(slug, token)
        .then((role) => setCanManage(role === "owner" || role === "admin"))
        .catch(() => setCanManage(false));
    };
    check();
    return subscribeToBearerTokenChange(check);
  }, [pathname]);

  useEffect(() => {
    let cancelled = false;

    async function loadBranding() {
      const hostTenantSlug =
        typeof window !== "undefined" ? subdomainSlug(window.location.host) : null;
      const pathTenantSlug = getTenantSlugFromPath(pathname);
      const queryTenantSlug =
        typeof window !== "undefined"
          ? new URLSearchParams(window.location.search).get("tenant")?.trim() || null
          : null;
      const tenantSlug = hostTenantSlug ?? pathTenantSlug ?? queryTenantSlug;

      if (!tenantSlug) {
        if (!cancelled) {
          setBranding(DEFAULT_BRANDING);
        }
        return;
      }

      try {
        const nextBranding = await getTenantBranding(tenantSlug);
        if (!cancelled) {
          setBranding(nextBranding);
        }
      } catch {
        if (!cancelled) {
          setBranding(DEFAULT_BRANDING);
        }
      }
    }

    loadBranding();
    return () => {
      cancelled = true;
    };
  }, [pathname]);

  useEffect(() => {
    if (!branding.logo_url) return;
    let icon = document.querySelector<HTMLLinkElement>("#tenant-brand-favicon");
    if (!icon) {
      icon = document.createElement("link");
      icon.id = "tenant-brand-favicon";
      icon.rel = "icon";
      document.head.appendChild(icon);
    }
    icon.href = branding.logo_url;
    return () => { icon?.remove(); };
  }, [branding.logo_url]);

  const shellStyle = useMemo(() => {
    const style: Record<string, string> = {};

    if (branding.accent_color) {
      style["--primary"] = branding.accent_color;
      style["--primary-dark"] = `color-mix(in srgb, ${branding.accent_color} 75%, #17252c)`;
      style["--primary-ink"] = contrastInk(branding.accent_color);
      style["--accent"] = branding.accent_color;
    }

    if (branding.background_color) {
      style["--tenant-bg"] = branding.background_color;
      style["--tenant-bg-soft"] = hexToRgba(branding.background_color, 0.52);
      style["--tenant-glow"] = hexToRgba(branding.background_color, 0.7);
    }

    return Object.keys(style).length > 0 ? (style as CSSProperties) : undefined;
  }, [branding.accent_color, branding.background_color]);

  const dashboardHref = buildTenantPath(
    "/dashboard",
    branding.tenant_slug,
    branding.tenant_slug,
  );
  const roadmapHref = buildTenantPath("/roadmap", branding.tenant_slug, branding.tenant_slug);
  const feedHref = buildTenantPath("/feed", branding.tenant_slug, branding.tenant_slug);
  const boardsHref = buildTenantPath("/", branding.tenant_slug, branding.tenant_slug);
  const myHref = buildTenantPath(
    "/my/account",
    branding.tenant_slug,
    branding.tenant_slug,
  );
  const homeHref = branding.tenant_slug
    ? buildTenantPath("/", branding.tenant_slug, branding.tenant_slug)
    : "/";
  const managerMode = pathname.startsWith("/app/");
  const appHref = branding.tenant_slug ? `/app/${encodeURIComponent(branding.tenant_slug)}` : "/";
  const externalAppHref = `${(process.env.NEXT_PUBLIC_HOWLLO_APP_URL || "http://localhost:7702").replace(/\/$/, "")}${appHref}`;
  const loginHome = pathname === "/" && !hasSession && ENABLED_AUTH_PROVIDERS.length === 1 && ENABLED_AUTH_PROVIDERS[0] === "rooiam";

  return (
    <div className={`app-shell app-shell--themed${loginHome ? " app-shell--login" : ""}${managerMode ? " app-shell--manager" : ""}`} style={shellStyle}>
      <header className={`site-header${loginHome ? " site-header--login" : ""}`}>
        <div className="site-header__inner">
          <div className="brand">
            <Link href={managerMode ? appHref : homeHref} className="brand__title" aria-label={managerMode ? "Howllo App" : branding.site_name}>
              {branding.logo_url ? (
                <img
                  alt=""
                  src={branding.logo_url}
                  className="brand__logo"
                  style={{ height: "2rem", maxWidth: "7rem" }}
                />
              ) : null}
              <span className="tenant-brand-wordmark">{branding.site_name}</span>
            </Link>
            <div className="brand__meta">
              {managerMode ? "Howllo App · Workspace management" : branding.site_name === branding.tenant_name
                ? "Feedback & feature requests, out in the open."
                : `${branding.tenant_name} feedback space`}
            </div>
          </div>
          {managerMode ? <nav className="nav" aria-label="Workspace management">
            <Link href={homeHref} className="nav__link">View public boards ↗</Link>
            <AuthControl myHref={myHref} />
          </nav> : !loginHome ? <nav className="nav">
            {branding.show_boards ? <Link href={boardsHref} className="nav__link">Boards</Link> : null}
            {branding.show_feed ? <Link href={feedHref} className="nav__link">Feed</Link> : null}
            <Link href={dashboardHref} className="nav__link">
              Dashboard
            </Link>
            {branding.show_roadmap ? <Link href={roadmapHref} className="nav__link">Roadmap</Link> : null}
            {canManage ? <a href={externalAppHref} className="nav__link">Howllo App ↗</a> : null}
            <NotificationBell />
            <AuthControl myHref={myHref} />
          </nav> : null}
        </div>
      </header>

      <main className="app-main">{children}</main>

      {branding.show_powered_by && !loginHome && !managerMode ? (
        <footer className="site-footer">
          <div className="site-footer__inner">
            <span>Powered by Howllo</span>
          </div>
        </footer>
      ) : null}
    </div>
  );
}
