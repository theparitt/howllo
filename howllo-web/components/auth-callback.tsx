"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import {
  createWorkspaceSession,
  getWorkspaceAuthConfig,
} from "@/lib/api";
import {
  ACTIVE_AUTH_PROVIDER,
  authProviderLabel,
} from "@/lib/auth-provider";
import {
  clearRooiamOidcState,
  consumeRooiamReturnTo,
  readRooiamReturnTo,
  readRooiamOidcState,
  writeRooiamOidcState,
} from "@/lib/rooiam-auth";
import { clearAllStoredBearerTokens, getWorkspaceSlugFromPath, persistBearerToken, writeAccountToken } from "@/components/dev-auth-panel";
import { subdomainSlug } from "@/lib/subdomain";
import { exchangeSignInCode } from "@/lib/auth-api";

function getTenantSlugFromReturnTo(returnTo: string) {
  try {
    const url = new URL(returnTo, window.location.origin);
    const queryTenant = url.searchParams.get("tenant")?.trim();
    if (queryTenant) return queryTenant;
    return getWorkspaceSlugFromPath(url.pathname) ?? subdomainSlug(url.host);
  } catch {
    return null;
  }
}

async function sha256Base64Url(input: string) {
  const bytes = new TextEncoder().encode(input);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const binary = Array.from(new Uint8Array(digest), (byte) => String.fromCharCode(byte)).join("");
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function randomBase64Url(bytes = 32) {
  const data = new Uint8Array(bytes);
  crypto.getRandomValues(data);
  const binary = Array.from(data, (byte) => String.fromCharCode(byte)).join("");
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

export function AuthCallback() {
  const router = useRouter();
  const [error, setError] = useState("");
  const [message, setMessage] = useState("Signing you in…");
  const [fallback, setFallback] = useState("/");
  const [genericCallback, setGenericCallback] = useState(false);
  const providerLabel = authProviderLabel(ACTIVE_AUTH_PROVIDER);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      const exchangeCode = new URLSearchParams(window.location.search).get("exchange_code");
      if (exchangeCode) {
        setGenericCallback(true);
        const result = await exchangeSignInCode(exchangeCode);
        const target = result.return_to.startsWith("/") && !result.return_to.startsWith("//") ? result.return_to : "/";
        const tenant = getTenantSlugFromReturnTo(target);
        const bearer = `Bearer ${result.access_token}`;
        writeAccountToken(bearer);
        if (tenant) {
          const session = await createWorkspaceSession({ tenantSlug: tenant, accessToken: result.access_token });
          persistBearerToken(tenant, `Bearer ${session.session_token}`);
        }
        if (!cancelled) router.replace(target);
        return;
      }
      if (ACTIVE_AUTH_PROVIDER !== "rooiam") {
        setError(`${providerLabel} callback is not implemented in howllo-web yet.`);
        return;
      }

      const params = new URLSearchParams(window.location.search);
      const nextFallback = readRooiamReturnTo() || "/";
      setFallback(nextFallback);
      const tenantSlug = getTenantSlugFromReturnTo(nextFallback);

      const authError = params.get("error") ?? "";
      if (authError) {
        setError(authError);
        return;
      }

      const redirectUri = `${window.location.origin}/auth/callback`;
      const code = params.get("code");
      const state = params.get("state");
      const existing = readRooiamOidcState();

      // Account mode (no workspace in the return path): sign in at the account
      // level using howllo's global rooiam app, to create/list workspaces.
      // Otherwise use the workspace's own rooiam config.
      let rooiamClientId = "";
      let rooiamBaseUrl = "";
      if (tenantSlug) {
        const workspaceAuth = await getWorkspaceAuthConfig(tenantSlug);
        if (workspaceAuth.provider !== "rooiam") {
          setError(`Workspace auth provider "${workspaceAuth.provider}" is not supported here.`);
          return;
        }
        rooiamClientId = workspaceAuth.rooiam_client_id?.trim() ?? "";
        rooiamBaseUrl = workspaceAuth.rooiam_widget_base_url?.trim() ?? "";
      } else {
        rooiamClientId = (process.env.NEXT_PUBLIC_ROOIAM_WIDGET_CLIENT_ID ?? "").trim();
        rooiamBaseUrl = (process.env.NEXT_PUBLIC_ROOIAM_WIDGET_BASE_URL ?? "").trim();
      }

      if (!code || !state) {
        if (!rooiamClientId) {
          setError("RooIAM client ID is missing for this app.");
          return;
        }
        if (!rooiamBaseUrl) {
          setError("RooIAM widget base URL is missing for this workspace.");
          return;
        }

        const nextState = randomBase64Url(24);
        const codeVerifier = randomBase64Url(48);
        const codeChallenge = await sha256Base64Url(codeVerifier);
        writeRooiamOidcState({
          state: nextState,
          codeVerifier,
          redirectUri,
        });
        const authorize = new URL(
          `${rooiamBaseUrl.replace(/\/login-widget\/?$/, "")}/v1/oidc/authorize`,
        );
        authorize.searchParams.set("client_id", rooiamClientId);
        authorize.searchParams.set("redirect_uri", redirectUri);
        authorize.searchParams.set("response_type", "code");
        authorize.searchParams.set("scope", "openid profile email");
        authorize.searchParams.set("state", nextState);
        authorize.searchParams.set("code_challenge", codeChallenge);
        authorize.searchParams.set("code_challenge_method", "S256");
        setMessage(`Finishing ${providerLabel} sign-in…`);
        window.location.replace(authorize.toString());
        return;
      }

      if (!existing) {
        setError(`Missing OIDC state for this ${providerLabel} callback.`);
        return;
      }

      if (state !== existing.state) {
        clearRooiamOidcState();
        setError(`${providerLabel} callback state mismatch.`);
        return;
      }

      setMessage(`Exchanging ${providerLabel} authorization code…`);
      const tokenResponse = await fetch("/api/auth/rooiam-token", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          code,
          codeVerifier: existing.codeVerifier,
          redirectUri: existing.redirectUri,
          tenantSlug,
        }),
      });
      const tokenPayload = await tokenResponse.json().catch(() => ({}));
      if (!tokenResponse.ok || typeof tokenPayload.access_token !== "string") {
        setError(
          typeof tokenPayload.error_description === "string"
            ? tokenPayload.error_description
            : typeof tokenPayload.message === "string"
              ? tokenPayload.message
              : `Could not exchange ${providerLabel} authorization code.`,
        );
        return;
      }

      if (tenantSlug) {
        const session = await createWorkspaceSession({
          tenantSlug,
          accessToken: tokenPayload.access_token,
        });
        persistBearerToken(tenantSlug, `Bearer ${session.session_token}`);
      } else {
        // Account mode: keep the rooiam token as the account credential for the
        // dashboard (list/create workspaces). Board sessions are minted per
        // workspace when the tenant opens one.
        clearAllStoredBearerTokens();
        writeAccountToken(`Bearer ${tokenPayload.access_token}`);
      }
      clearRooiamOidcState();
      consumeRooiamReturnTo();

      if (!cancelled) {
        setMessage("Signed in. Returning you to the board…");
        router.replace(nextFallback);
      }
    }

    void run().catch((cause) => {
      if (!cancelled) {
        setError(cause instanceof Error ? cause.message : "Could not complete sign-in.");
      }
    });

    return () => {
      cancelled = true;
    };
  }, [providerLabel, router]);

  if (error) {
    return (
      <div className="page-stack">
        <section className="panel empty-state workspace-state">
          <span className="kicker">{genericCallback ? "Howllo" : providerLabel} callback</span>
          <h1 className="empty-state__title">Sign-in did not complete.</h1>
          <p className="empty-state__copy">
            {genericCallback ? "Sign-in failed" : `${providerLabel} returned an error`}: <code>{error}</code>
          </p>
          <div className="hero__actions">
            <Link className="button" href={fallback}>
              Return
            </Link>
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="page-stack">
      <section className="panel empty-state workspace-state">
        <span className="kicker">{providerLabel} callback</span>
        <h1 className="empty-state__title">Completing sign-in</h1>
        <p className="empty-state__copy">{message}</p>
        <p className="workspace-state__hint">
          If nothing happens, <Link href={fallback}>continue here</Link>.
        </p>
      </section>
    </div>
  );
}
