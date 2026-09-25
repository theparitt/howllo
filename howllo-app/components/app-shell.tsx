"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { AuthControl } from "@/components/auth-control";

const publicWebUrl = (process.env.NEXT_PUBLIC_HOWLLO_PUBLIC_WEB_URL || "http://localhost:7703").replace(/\/$/, "");

export function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const tenantSlug = pathname.startsWith("/app/") ? pathname.split("/")[2] : null;
  return <div className="app-shell app-shell--themed app-shell--manager">
    <header className="site-header"><div className="site-header__inner">
      <div className="brand">
        <div className="app-brand-line"><Link href="/" className="brand__title" aria-label="Howllo Staff App"><img alt="Howllo" src="/brand/howllo-logo-wordmark-horizontal.svg" className="brand__logo" style={{ height: "1.7rem" }} /></Link><span className="app-brand-badge">STAFF APP</span></div>
        <div className="brand__meta">Manage workspaces and public boards</div>
      </div>
      <nav className="nav" aria-label="App navigation">
        {tenantSlug ? <a className="nav__link" href={`${publicWebUrl}/${encodeURIComponent(tenantSlug)}`}>View public boards ↗</a> : null}
        <AuthControl />
      </nav>
    </div></header>
    <main className="app-main">{children}</main>
  </div>;
}
