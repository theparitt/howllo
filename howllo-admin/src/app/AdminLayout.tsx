import { NavLink, Outlet, useLocation } from "react-router-dom";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { adminRoutes } from "@howllo/config";
import { SessionBar } from "./SessionBar";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";
import { DebugBadge } from "./DebugBadge";
import { AuthGate } from "../features/auth/AuthGate";
import { useSession } from "../lib/session";
import { useEscapeKey, useFocusTrap } from "../lib/a11y";

type NavItem = { to: string; label: string; icon: ReactNode; end?: boolean };

// Minimal stroke icons (no icon dependency).
const icon = (path: string): ReactNode => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden>
    <path
      d={path}
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

const PLATFORM_NAV: NavItem[] = [
  { to: adminRoutes.home(), label: "Dashboard", end: true, icon: icon("M3 12l9-8 9 8M5 10v10h14V10") },
  { to: adminRoutes.tenants(), label: "Workspaces", icon: icon("M3 7h18M3 12h18M3 17h18") },
  { to: adminRoutes.users(), label: "User accounts", icon: icon("M16 19v-1a4 4 0 00-8 0v1M12 11a3 3 0 100-6 3 3 0 000 6M19 8h3M20.5 6.5v3") },
  { to: adminRoutes.platform(), label: "Platform", icon: icon("M4 6h16M4 12h16M4 18h16M8 6v12") },
];

const WORKSPACE_NAV: NavItem[] = [
  { to: adminRoutes.boards(), label: "Boards", icon: icon("M4 5h16v14H4zM4 10h16M10 10v9") },
  { to: adminRoutes.roadmap(), label: "Roadmap", icon: icon("M4 6h10M4 12h16M4 18h7") },
  { to: adminRoutes.moderation(), label: "Content", icon: icon("M5 4h14v16l-7-4-7 4z") },
  { to: adminRoutes.tags(), label: "Tags", icon: icon("M4 4h7l9 9-7 7-9-9zM8 8h.01") },
  { to: adminRoutes.members(), label: "Members", icon: icon("M16 19v-1a4 4 0 00-8 0v1M12 11a3 3 0 100-6 3 3 0 000 6") },
  { to: adminRoutes.integrations(), label: "Integrations", icon: icon("M10 13a5 5 0 007 0l2-2a5 5 0 00-7-7l-1 1M14 11a5 5 0 00-7 0l-2 2a5 5 0 007 7l1-1") },
  { to: adminRoutes.audit(), label: "Audit", icon: icon("M12 8v4l3 2M12 21a9 9 0 110-18 9 9 0 010 18z") },
  { to: adminRoutes.settings(), label: "Settings", icon: icon("M12 15a3 3 0 100-6 3 3 0 000 6zM19 12a7 7 0 00-.1-1l2-1.5-2-3.5-2.4 1a7 7 0 00-1.7-1l-.3-2.5h-4l-.3 2.5a7 7 0 00-1.7 1l-2.4-1-2 3.5L4.1 11a7 7 0 000 2l-2 1.5 2 3.5 2.4-1a7 7 0 001.7 1l.3 2.5h4l.3-2.5a7 7 0 001.7-1l2.4 1 2-3.5-2-1.5a7 7 0 00.1-1z") },
];

function NavList({ items }: { items: NavItem[] }) {
  return (
    <nav className="nav-list">
      {items.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.end}
          className={({ isActive }) =>
            isActive ? "admin-nav-link active" : "admin-nav-link"
          }
        >
          <span className="nav-icon">{item.icon}</span>
          {item.label}
        </NavLink>
      ))}
    </nav>
  );
}

export function AdminLayout() {
  const { isAuthenticated, loading } = useSession();
  const [navOpen, setNavOpen] = useState(false);
  const navRef = useRef<HTMLElement>(null);
  const location = useLocation();

  useEscapeKey(navOpen, () => setNavOpen(false));
  useFocusTrap(navOpen, navRef);

  // Close the mobile drawer whenever the route changes.
  useEffect(() => {
    setNavOpen(false);
  }, [location.pathname]);

  if (loading) {
    return <div className="auth-screen" />;
  }

  if (!isAuthenticated) {
    return <AuthGate />;
  }

  return (
    <div className={`admin-shell${navOpen ? " admin-shell--nav-open" : ""}`}>
      {/* Mobile top bar with the menu toggle (hidden on desktop via CSS). */}
      <header className="mobile-topbar">
        <button
          className="mobile-menu-btn"
          aria-label="Open menu"
          onClick={() => setNavOpen(true)}
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
            <path d="M4 7h16M4 12h16M4 17h16" />
          </svg>
        </button>
        <img
          className="mobile-topbar__logo"
          src="/brand/howllo-logo-wordmark-horizontal.svg"
          alt="Howllo"
        />
      </header>

      {/* Backdrop behind the open drawer (mobile only). */}
      <div className="sidebar-backdrop" onClick={() => setNavOpen(false)} />

      <aside className="admin-sidebar" ref={navRef} tabIndex={-1}>
        <div className="admin-brand">
          <img
            className="admin-brand-mark"
            src="/brand/howllo-logo-wordmark-horizontal.svg"
            alt="Howllo"
          />
          <span className="admin-brand-sub">Admin</span>
          <button
            className="sidebar-close"
            aria-label="Close menu"
            onClick={() => setNavOpen(false)}
          >
            x
          </button>
        </div>

        <WorkspaceSwitcher />

        <div className="sidebar-scroll">
          <div className="sidebar-section">
            <div className="sidebar-heading">Platform</div>
            <NavList items={PLATFORM_NAV} />
          </div>

          <div className="sidebar-section">
            <div className="sidebar-heading">Workspace</div>
            <NavList items={WORKSPACE_NAV} />
          </div>
        </div>
      </aside>

      <div className="admin-content">
        <SessionBar />
        <main className="admin-main">
          <Outlet />
        </main>
      </div>

      <DebugBadge />
    </div>
  );
}
