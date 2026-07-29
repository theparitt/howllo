"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { type CSSProperties, useEffect, useMemo, useState } from "react";
import { AuthControl } from "@/components/auth-control";
import { NotificationBell } from "@/components/notification-bell";
import { getMyWorkspaceRole, getTenantBranding } from "@/lib/api";
import { readStoredBearerToken, subscribeToBearerTokenChange } from "@/components/dev-auth-panel";
import type { TenantBranding } from "@/lib/types";
import { buildTenantPath } from "@/lib/default-tenant";
import { hexToRgba } from "@/lib/theme";

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
};

function getTenantSlugFromPath(pathname: string): string | null {
  const [first] = pathname.replace(/^\/+/, "").split("/");
  if (RESERVED_TOP_LEVEL_ROUTES.has(first ?? "")) {
    return null;
  }
  return first || null;
}

export function TenantShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const [branding, setBranding] = useState<TenantBranding>(DEFAULT_BRANDING);
  const [canManage, setCanManage] = useState(false);

  // Show the Manage link only to workspace owners/admins.
  useEffect(() => {
    const check = () => {
      const slug =
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
      const pathTenantSlug = getTenantSlugFromPath(pathname);
      const queryTenantSlug =
        typeof window !== "undefined"
          ? new URLSearchParams(window.location.search).get("tenant")?.trim() || null
          : null;
      const tenantSlug = pathTenantSlug ?? queryTenantSlug;

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

  const shellStyle = useMemo(() => {
    const style: Record<string, string> = {};

    if (branding.accent_color) {
      style["--primary"] = branding.accent_color;
      style["--primary-dark"] = branding.accent_color;
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
  const manageHref = buildTenantPath("/manage", branding.tenant_slug, branding.tenant_slug);
  const myHref = buildTenantPath(
    "/my/account",
    branding.tenant_slug,
    branding.tenant_slug,
  );
  const homeHref = branding.tenant_slug
    ? buildTenantPath("/", branding.tenant_slug, branding.tenant_slug)
    : "/";

  return (
    <div className="app-shell app-shell--themed" style={shellStyle}>
      <header className="site-header">
        <div className="site-header__inner">
          <div className="brand">
            <Link href={homeHref} className="brand__title" aria-label={branding.site_name}>
              {branding.logo_url ? (
                <img
                  alt={branding.site_name}
                  src={branding.logo_url}
                  className="brand__logo"
                  style={{ height: "2rem" }}
                />
              ) : (
                <img
                  alt="Howllo"
                  src="/brand/howllo-logo-wordmark-horizontal.svg"
                  className="brand__logo"
                  style={{ height: "1.7rem" }}
                />
              )}
            </Link>
            <div className="brand__meta">
              {branding.site_name === branding.tenant_name
                ? "Feedback & feature requests, out in the open."
                : `${branding.tenant_name} feedback space`}
            </div>
          </div>
          <nav className="nav">
            <Link href={feedHref} className="nav__link">
              Feed
            </Link>
            <Link href={dashboardHref} className="nav__link">
              Dashboard
            </Link>
            {branding.show_roadmap ? (
              <Link href={roadmapHref} className="nav__link">
                Roadmap
              </Link>
            ) : null}
            {canManage ? (
              <Link href={manageHref} className="nav__link">
                Manage
              </Link>
            ) : null}
            <NotificationBell />
            <AuthControl myHref={myHref} />
          </nav>
        </div>
      </header>

      <main className="app-main">{children}</main>

      {branding.show_powered_by ? (
        <footer className="site-footer">
          <div className="site-footer__inner">
            <span>Powered by Howllo</span>
          </div>
        </footer>
      ) : null}
    </div>
  );
}
