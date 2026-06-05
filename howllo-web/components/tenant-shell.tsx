"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { type CSSProperties, useEffect, useMemo, useState } from "react";
import { RooiamLoginWidget } from "@/components/rooiam-login-widget";
import { getBootstrapTenant, getTenantBranding } from "@/lib/api";
import type { TenantBranding } from "@/lib/types";
import { buildTenantPath } from "@/lib/default-tenant";

const RESERVED_TOP_LEVEL_ROUTES = new Set([
  "",
  "dashboard",
  "boards",
  "posts",
  "roadmap",
  "admin",
]);

const DEFAULT_BRANDING: TenantBranding = {
  tenant_slug: "",
  tenant_name: "Howllo",
  site_name: "Howllo",
  logo_url: null,
  accent_color: null,
  show_powered_by: true,
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
  const [defaultTenantSlug, setDefaultTenantSlug] = useState("");

  useEffect(() => {
    let cancelled = false;

    async function loadBranding() {
      try {
        const bootstrap = await getBootstrapTenant();
        if (cancelled) return;

        const pathTenantSlug = getTenantSlugFromPath(pathname);
        const tenantSlug = pathTenantSlug ?? bootstrap.default_tenant_slug;
        setDefaultTenantSlug(bootstrap.default_tenant_slug);

        const nextBranding = await getTenantBranding(tenantSlug);
        if (!cancelled) {
          setBranding(nextBranding);
        }
      } catch {
        if (!cancelled) {
          setBranding(DEFAULT_BRANDING);
          setDefaultTenantSlug("");
        }
      }
    }

    loadBranding();
    return () => {
      cancelled = true;
    };
  }, [pathname]);

  const shellStyle = useMemo(() => {
    if (!branding.accent_color) return undefined;
    return {
      "--primary": branding.accent_color,
      "--primary-dark": branding.accent_color,
      "--accent": branding.accent_color,
    } as CSSProperties;
  }, [branding.accent_color]);

  const dashboardHref = buildTenantPath(
    "/dashboard",
    branding.tenant_slug,
    defaultTenantSlug,
  );
  const roadmapHref = buildTenantPath(
    "/roadmap",
    branding.tenant_slug,
    defaultTenantSlug,
  );
  const homeHref = buildTenantPath("/", branding.tenant_slug, defaultTenantSlug);

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
            <Link href={dashboardHref} className="nav__link">
              Dashboard
            </Link>
            <Link href={roadmapHref} className="nav__link">
              Roadmap
            </Link>
            <RooiamLoginWidget />
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
