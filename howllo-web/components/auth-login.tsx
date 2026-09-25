"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ApiError, createWorkspaceSession, getMe, getWorkspaceAuthConfig, revokeWorkspaceSession } from "@/lib/api";
import { getLoginProviders, localSignIn, logoutAccount, resetLocalPassword, type LoginProvider } from "@/lib/auth-api";
import { API_BASE_URL } from "@/lib/config";
import { ENABLED_AUTH_PROVIDERS } from "@/lib/auth-provider";
import { rememberRooiamReturnTo } from "@/lib/rooiam-auth";
import {
  clearAllStoredBearerTokens,
  clearStoredBearerToken,
  getCurrentWorkspaceSlug,
  readAnyStoredBearerToken,
  readStoredBearerToken,
  subscribeToBearerTokenChange,
  persistBearerToken,
  readAccountToken,
  writeAccountToken,
} from "@/components/dev-auth-panel";

type AuthLoginProps = {
  /** Href for the signed-in "My account" menu item. */
  myHref?: string;
};

const OPEN_LOGIN_EVENT = "howllo:open-login";

export function requestLogin() {
  if (typeof window !== "undefined") window.dispatchEvent(new Event(OPEN_LOGIN_EVENT));
}

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

export function AuthLogin({ myHref }: AuthLoginProps) {
  const pathname = usePathname();
  const [loginOpen, setLoginOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [signedIn, setSignedIn] = useState(false);
  const [tenantSlug, setTenantSlug] = useState<string | null>(null);
  const [displayName, setDisplayName] = useState<string>("");
  const [widgetUrl, setWidgetUrl] = useState("");
  const [widgetOrigin, setWidgetOrigin] = useState<string | null>(null);
  const [widgetHeight, setWidgetHeight] = useState(520);
  const [configured, setConfigured] = useState(false);
  const [providers, setProviders] = useState<LoginProvider[]>([]);
  const [providerError, setProviderError] = useState("");
  const [localOpen, setLocalOpen] = useState(false);
  const [legacyOpen, setLegacyOpen] = useState(false);
  const [registerMode, setRegisterMode] = useState(false);
  const [resetMode, setResetMode] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [recoveryInput, setRecoveryInput] = useState("");
  const [recoveryCode, setRecoveryCode] = useState("");
  const [pendingToken, setPendingToken] = useState("");
  const [busy, setBusy] = useState(false);
  const profileRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getLoginProviders().then(setProviders).catch(() => setProviderError("Could not load sign-in options. Please try again later."));
  }, []);

  useEffect(() => {
    const workspace = getCurrentWorkspaceSlug();
    setTenantSlug(workspace);
    setSignedIn(Boolean(workspace ? readStoredBearerToken(workspace).trim() : readAnyStoredBearerToken().trim()));
  }, [pathname]);

  useEffect(() => {
    if (!loginOpen || !legacyOpen || !widgetOrigin) return;
    function onMessage(event: MessageEvent) {
      if (!widgetOrigin || event.origin !== widgetOrigin) {
        return;
      }
      if (event.data?.type === "rooiam-login-widget:size") {
        const height = Number(event.data.height);
        if (Number.isFinite(height) && height >= 300 && height <= 2000) {
          setWidgetHeight(Math.max(420, Math.min(680, Math.ceil(height))));
        }
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
  }, [widgetOrigin, loginOpen, legacyOpen]);

  useEffect(() => {
    const open = () => {
      rememberRooiamReturnTo(`${window.location.pathname}${window.location.search}`);
      setLoginOpen(true);
    };
    window.addEventListener(OPEN_LOGIN_EVENT, open);
    return () => window.removeEventListener(OPEN_LOGIN_EVENT, open);
  }, []);

  useEffect(() => {
    if (!loginOpen) return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setLoginOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [loginOpen]);

  useEffect(() => {
    if (loginOpen && ENABLED_AUTH_PROVIDERS.includes("rooiam") && configured && providers.length === 0) {
      setLegacyOpen(true);
      setLocalOpen(false);
    } else if (loginOpen && ENABLED_AUTH_PROVIDERS.length === 1 && ENABLED_AUTH_PROVIDERS[0] === "local" && providers.length === 1 && providers[0]?.kind === "local") {
      setLocalOpen(true);
      setLegacyOpen(false);
    }
  }, [loginOpen, configured, providers.length]);

  useEffect(() => {
    if (!ENABLED_AUTH_PROVIDERS.includes("rooiam")) return;
    let cancelled = false;

    async function loadWorkspaceAuth() {
      const workspace = getCurrentWorkspaceSlug();
      if (!workspace) {
        const base = process.env.NEXT_PUBLIC_ROOIAM_WIDGET_BASE_URL?.trim();
        const workspaceId = process.env.NEXT_PUBLIC_ROOIAM_WIDGET_WORKSPACE_ID?.trim();
        const clientId = process.env.NEXT_PUBLIC_ROOIAM_WIDGET_CLIENT_ID?.trim();
        let url = "";
        if (base && workspaceId && clientId) {
          try {
            url = buildWidgetUrl({
              rooiam_widget_base_url: base,
              rooiam_workspace_id: workspaceId,
              rooiam_client_id: clientId,
            });
          } catch {
            url = "";
          }
        }
        if (!cancelled) {
          setWidgetUrl(url);
          setWidgetOrigin(url ? new URL(url).origin : null);
          setConfigured(Boolean(url));
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
      setSignedIn(Boolean(workspace ? readStoredBearerToken(workspace).trim() : readAnyStoredBearerToken().trim()));
    };
    sync();
    return subscribeToBearerTokenChange(sync);
  }, []);

  useEffect(() => {
    if (!signedIn) {
      setDisplayName("");
      return;
    }
    const token = (tenantSlug ? readStoredBearerToken(tenantSlug) : readAnyStoredBearerToken()).trim();
    if (!token) return;
    let cancelled = false;
    getMe(token)
      .then((user) => {
        if (!cancelled) setDisplayName(user.display_name || user.email || "Account");
      })
      .catch((error) => {
        if (cancelled) return;
        if (error instanceof ApiError && error.status === 401) {
          if (tenantSlug) clearStoredBearerToken(tenantSlug);
          else clearAllStoredBearerTokens();
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
    }
    const accountToken = readAccountToken().trim();
    if (accountToken) {
      try { await logoutAccount(accountToken); } catch {}
    }
    clearAllStoredBearerTokens();
    setMenuOpen(false);
  }

  async function submitLocal(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if ((registerMode || resetMode) && password !== confirmPassword) {
      setProviderError("Passwords do not match.");
      return;
    }
    setBusy(true);
    setProviderError("");
    try {
      const result = resetMode
        ? await resetLocalPassword(username, recoveryInput, password)
        : await localSignIn(registerMode ? "register" : "login", username, password);
      setPassword("");
      setConfirmPassword("");
      setRecoveryInput("");
      if (result.recovery_code) {
        setPendingToken(result.access_token);
        setRecoveryCode(result.recovery_code);
      } else {
        await completeLocalSignIn(result.access_token);
      }
    } catch (error) {
      setProviderError(error instanceof Error ? error.message : "Could not sign in.");
    } finally {
      setBusy(false);
    }
  }

  async function completeLocalSignIn(accessToken: string) {
    if (tenantSlug) {
      const session = await createWorkspaceSession({ tenantSlug, accessToken });
      persistBearerToken(tenantSlug, `Bearer ${session.session_token}`);
    }
    writeAccountToken(`Bearer ${accessToken}`);
    setLoginOpen(false);
    window.location.reload();
  }

  async function continueAfterRecoveryCode() {
    setBusy(true);
    setProviderError("");
    try {
      await completeLocalSignIn(pendingToken);
    } catch (error) {
      setProviderError(error instanceof Error ? error.message : "Could not finish sign-in.");
    } finally {
      setBusy(false);
    }
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
            {myHref && tenantSlug ? (
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

  // The home page already shows the RooIAM widget when it is the only provider.
  if ((pathname === "/" || pathname.startsWith("/app/")) && ENABLED_AUTH_PROVIDERS.length === 1 && ENABLED_AUTH_PROVIDERS[0] === "rooiam") {
    return null;
  }

  // Signed-out: sign-in button + login widget.
  return (
    <>
      <button className="button" type="button" onClick={openLogin}>
        {loginOpen ? "Hide login" : "Sign in"}
      </button>

      {loginOpen && typeof document !== "undefined" ? createPortal(
        <div
          className="widget-overlay"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) setLoginOpen(false);
          }}
        >
          <div className="widget-shell" role="dialog" aria-modal="true" aria-label="Sign in">
          <button
            className="widget-shell__close"
            type="button"
            aria-label="Close"
            onClick={() => setLoginOpen(false)}
          >
            ×
          </button>

          <div style={{ padding: "1.6rem", display: "grid", gap: "0.75rem", width: "min(420px, calc(100vw - 2rem))" }}>
            <h2 style={{ margin: 0 }}>Sign in to Howllo</h2>
            {!recoveryCode && !localOpen && !legacyOpen && providers.map((provider) => provider.kind === "oidc" ? (
              <button key={provider.id} className="button" type="button" onClick={() => {
                const url = new URL(`${API_BASE_URL}${provider.login_url}`);
                url.searchParams.set("return_to", `${window.location.pathname}${window.location.search}`);
                window.location.assign(url.toString());
              }}>Continue with {provider.display_name}</button>
            ) : (
              <button key={provider.id} className="button" type="button" onClick={() => { setLocalOpen(true); setLegacyOpen(false); }}>
                Continue with {provider.display_name}
              </button>
            ))}
            {!recoveryCode && !legacyOpen && ENABLED_AUTH_PROVIDERS.includes("rooiam") && configured && widgetUrl && !providers.some((provider) => provider.id === "rooiam") ? (
              <button className="button" type="button" onClick={() => { setLegacyOpen(true); setLocalOpen(false); }}>Continue with RooIAM</button>
            ) : null}
            {recoveryCode ? (
              <div className="stack" role="status">
                <h3 style={{ margin: 0 }}>Save your recovery code</h3>
                <p>This code is shown only once. Store it somewhere safe. You will need it if you forget your password.</p>
                <code style={{ overflowWrap: "anywhere", userSelect: "all" }}>{recoveryCode}</code>
                <button className="button" type="button" onClick={() => void navigator.clipboard.writeText(recoveryCode)}>Copy code</button>
                <button className="button button--cta" type="button" disabled={busy} onClick={() => void continueAfterRecoveryCode()}>I saved the code — continue</button>
              </div>
            ) : null}
            {localOpen && !recoveryCode ? (
              <form onSubmit={(event) => void submitLocal(event)} style={{ display: "grid", gap: "0.6rem" }}>
                <label htmlFor="howllo-username">Username</label>
                <input id="howllo-username" className="field" autoComplete="username" value={username} onChange={(event) => setUsername(event.target.value)} required />
                {resetMode ? <><label htmlFor="howllo-recovery">Recovery code</label>
                  <input id="howllo-recovery" className="field" autoComplete="off" value={recoveryInput} onChange={(event) => setRecoveryInput(event.target.value)} required /></> : null}
                <label htmlFor="howllo-password">{resetMode ? "New password" : "Password"}</label>
                <input id="howllo-password" className="field" type="password" autoComplete={registerMode || resetMode ? "new-password" : "current-password"} value={password} onChange={(event) => setPassword(event.target.value)} required minLength={registerMode || resetMode ? 12 : undefined} />
                {registerMode || resetMode ? <><label htmlFor="howllo-confirm-password">Confirm password</label>
                  <input id="howllo-confirm-password" className="field" type="password" autoComplete="new-password" value={confirmPassword} onChange={(event) => setConfirmPassword(event.target.value)} required minLength={12} /></> : null}
                <button className="button button--cta" disabled={busy} type="submit">{resetMode ? "Reset password" : registerMode ? "Create account" : "Sign in"}</button>
                <button className="button" type="button" onClick={() => { const creating = !registerMode && !resetMode; setResetMode(false); setRegisterMode(creating); setPassword(""); }}>{registerMode || resetMode ? "Back to sign in" : "Create a local account"}</button>
                {!registerMode ? <button className="button" type="button" onClick={() => { setResetMode((value) => !value); setPassword(""); }}>{resetMode ? "Use password instead" : "Forgot password? Use recovery code"}</button> : null}
              </form>
            ) : null}
            {providerError ? <p role="alert" className="notice notice--error">{providerError}</p> : null}
            {!providers.length && !configured && !providerError ? <p>No sign-in provider is configured. Contact your administrator.</p> : null}
          </div>
          {legacyOpen && configured && widgetUrl ? (
            <iframe
              key={widgetUrl}
              title="Sign in"
              src={widgetUrl}
              width="420"
              height="520"
              allow="publickey-credentials-get *"
              className="widget-shell__frame"
              style={{ height: `min(${widgetHeight}px, calc(100dvh - 2rem))` }}
            />
          ) : null}
          </div>
        </div>,
        document.body,
      ) : null}
    </>
  );
}
