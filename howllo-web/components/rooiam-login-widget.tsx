"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { ApiError, getMe, getWorkspaceAuthConfig, revokeWorkspaceSession } from "@/lib/api";
import { rememberRooiamReturnTo } from "@/lib/rooiam-auth";
import {
  clearStoredBearerToken,
  getCurrentWorkspaceSlug,
  readStoredBearerToken,
  subscribeToBearerTokenChange,
} from "@/components/dev-auth-panel";

type RooiamLoginWidgetProps = {
  /** Href for the signed-in "My account" menu item. */
  myHref?: string;
};

function buildWidgetUrl(config: {
  rooiam_widget_base_url: string | null;
  rooiam_workspace_id: string | null;
  rooiam_client_id: string | null;
}) {
  if (!config.rooiam_widget_base_url || !config.rooiam_workspace_id || !config.rooiam_client_id) {
    return "";
  }
  const url = new URL(config.rooiam_widget_base_url);
  url.searchParams.set("workspace_id", config.rooiam_workspace_id);
  url.searchParams.set("client_id", config.rooiam_client_id);
  return url.toString();
}

function initialOf(name: string) {
  const trimmed = name.trim();
  return trimmed ? trimmed[0]!.toUpperCase() : "?";
}

export function RooiamLoginWidget({ myHref }: RooiamLoginWidgetProps) {
  const pathname = usePathname();
  const [loginOpen, setLoginOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [signedIn, setSignedIn] = useState(false);
  const [tenantSlug, setTenantSlug] = useState<string | null>(null);
  const [displayName, setDisplayName] = useState<string>("");
  const [widgetUrl, setWidgetUrl] = useState("");
  const [widgetOrigin, setWidgetOrigin] = useState<string | null>(null);
  const [configured, setConfigured] = useState(false);
  const profileRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const workspace = getCurrentWorkspaceSlug();
    setTenantSlug(workspace);
    setSignedIn(Boolean(workspace && readStoredBearerToken(workspace).trim()));
  }, [pathname]);

  useEffect(() => {
    function onMessage(event: MessageEvent) {
      if (!widgetOrigin || event.origin !== widgetOrigin) {
        return;
      }
      if (event.data?.type !== "rooiam:navigate" || typeof event.data.url !== "string") {
        return;
      }
      rememberRooiamReturnTo(`${window.location.pathname}${window.location.search}`);
      window.location.href = event.data.url;
    }

    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [widgetOrigin]);

  useEffect(() => {
    let cancelled = false;

    async function loadWorkspaceAuth() {
      const workspace = getCurrentWorkspaceSlug();
      if (!workspace) {
        if (!cancelled) {
          setWidgetUrl("");
          setWidgetOrigin(null);
          setConfigured(false);
        }
        return;
      }

      try {
        const config = await getWorkspaceAuthConfig(workspace);
        if (config.provider !== "rooiam") {
          if (!cancelled) {
            setWidgetUrl("");
            setWidgetOrigin(null);
            setConfigured(false);
          }
          return;
        }

        const url = buildWidgetUrl(config);
        if (!cancelled) {
          setWidgetUrl(url);
          setWidgetOrigin(url ? new URL(url).origin : null);
          setConfigured(Boolean(url));
        }
      } catch {
        if (!cancelled) {
          setWidgetUrl("");
          setWidgetOrigin(null);
          setConfigured(false);
        }
      }
    }

    void loadWorkspaceAuth();
    return () => {
      cancelled = true;
    };
  }, [pathname]);

  useEffect(() => {
    const sync = () => {
      const workspace = getCurrentWorkspaceSlug();
      setTenantSlug(workspace);
      setSignedIn(Boolean(workspace && readStoredBearerToken(workspace).trim()));
    };
    sync();
    return subscribeToBearerTokenChange(sync);
  }, []);

  useEffect(() => {
    if (!signedIn || !tenantSlug) {
      setDisplayName("");
      return;
    }
    const token = readStoredBearerToken(tenantSlug).trim();
    if (!token) return;
    let cancelled = false;
    getMe(token)
      .then((user) => {
        if (!cancelled) setDisplayName(user.display_name || user.email || "Account");
      })
      .catch((error) => {
        if (cancelled) return;
        if (error instanceof ApiError && error.status === 401) {
          clearStoredBearerToken(tenantSlug);
          setSignedIn(false);
          setDisplayName("");
          return;
        }
        setDisplayName("Account");
      });
    return () => {
      cancelled = true;
    };
  }, [signedIn, tenantSlug]);

  // Close the profile dropdown on outside click / Escape.
  useEffect(() => {
    if (!menuOpen) return;
    function onClick(event: MouseEvent) {
      if (profileRef.current && !profileRef.current.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    }
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") setMenuOpen(false);
    }
    document.addEventListener("mousedown", onClick);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onClick);
      document.removeEventListener("keydown", onKey);
    };
  }, [menuOpen]);

  function openLogin() {
    if (!loginOpen) {
      rememberRooiamReturnTo(`${window.location.pathname}${window.location.search}`);
    }
    setLoginOpen((value) => !value);
  }

  async function logout() {
    if (tenantSlug) {
      const token = readStoredBearerToken(tenantSlug).trim();
      if (token) {
        try {
          await revokeWorkspaceSession(token);
        } catch {}
      }
      clearStoredBearerToken(tenantSlug);
    }
    setMenuOpen(false);
  }

  // Signed-in: profile button + dropdown (My account, Log out).
  if (signedIn) {
    const label = displayName || "Account";
    return (
      <div className="profile-menu" ref={profileRef}>
        <button
          className="profile-trigger"
          type="button"
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          onClick={() => setMenuOpen((value) => !value)}
        >
          <span className="profile-avatar" aria-hidden="true">
            {initialOf(label)}
          </span>
          <span className="profile-name">{label}</span>
          <span className="profile-caret" aria-hidden="true">
            ⌄
          </span>
        </button>

        {menuOpen ? (
          <div className="profile-dropdown" role="menu">
            {myHref ? (
              <Link
                href={myHref}
                className="profile-dropdown__item"
                role="menuitem"
                onClick={() => setMenuOpen(false)}
              >
                My account
              </Link>
            ) : null}
            <button
              className="profile-dropdown__item profile-dropdown__item--danger"
              type="button"
              role="menuitem"
              onClick={() => void logout()}
            >
              Log out
            </button>
          </div>
        ) : null}
      </div>
    );
  }

  // Signed-out: sign-in button + login widget.
  return (
    <>
      <button className="button" type="button" onClick={openLogin}>
        {loginOpen ? "Hide login" : "Sign in"}
      </button>

      {loginOpen ? (
        <div className="widget-shell" role="dialog" aria-modal="false" aria-label="Sign in">
          <button
            className="widget-shell__close"
            type="button"
            aria-label="Close"
            onClick={() => setLoginOpen(false)}
          >
            ×
          </button>

          {configured && widgetUrl ? (
            <iframe
              key={widgetUrl}
              title="Sign in"
              src={widgetUrl}
              width="420"
              height="520"
              allow="publickey-credentials-get *"
              className="widget-shell__frame"
            />
          ) : (
            <div className="notice notice--error">
              RooIAM login is not configured for this workspace yet.
            </div>
          )}
        </div>
      ) : null}
    </>
  );
}
