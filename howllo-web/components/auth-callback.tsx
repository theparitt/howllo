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
import { getWorkspaceSlugFromPath, persistBearerToken } from "@/components/dev-auth-panel";

function getTenantSlugFromReturnTo(returnTo: string) {
  try {
    const url = new URL(returnTo, window.location.origin);
    const queryTenant = url.searchParams.get("tenant")?.trim();
    if (queryTenant) return queryTenant;
    return getWorkspaceSlugFromPath(url.pathname);
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
  const providerLabel = authProviderLabel(ACTIVE_AUTH_PROVIDER);

  useEffect(() => {
    let cancelled = false;

    async function run() {
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
      if (!tenantSlug) {
        setError("Open a workspace before completing sign-in.");
        return;
      }
      const workspaceAuth = await getWorkspaceAuthConfig(tenantSlug);
      if (workspaceAuth.provider !== "rooiam") {
        setError(`Workspace auth provider "${workspaceAuth.provider}" is not supported here.`);
        return;
      }
      const rooiamClientId = workspaceAuth.rooiam_client_id?.trim() ?? "";
      const rooiamBaseUrl = workspaceAuth.rooiam_widget_base_url?.trim() ?? "";

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
      const tokenBody = new URLSearchParams({
        grant_type: "authorization_code",
        code,
        redirect_uri: existing.redirectUri,
        client_id: rooiamClientId,
        code_verifier: existing.codeVerifier,
      });
      const tokenResponse = await fetch(
        `${rooiamBaseUrl.replace(/\/login-widget\/?$/, "")}/v1/oidc/token`,
        {
          method: "POST",
          headers: {
            "content-type": "application/x-www-form-urlencoded",
          },
          body: tokenBody.toString(),
        },
      );
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

      const session = await createWorkspaceSession({
        tenantSlug,
        rooiamAccessToken: tokenPayload.access_token,
      });
      persistBearerToken(tenantSlug, `Bearer ${session.session_token}`);
      clearRooiamOidcState();
      consumeRooiamReturnTo();

      if (!cancelled) {
        setMessage("Signed in. Returning you to the board…");
        router.replace(nextFallback);
      }
    }

    void run().catch((cause) => {
      if (!cancelled) {
        setError(cause instanceof Error ? cause.message : "Could not complete RooIAM sign-in.");
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
          <span className="kicker">{providerLabel} callback</span>
          <h1 className="empty-state__title">Sign-in did not complete.</h1>
          <p className="empty-state__copy">
            {providerLabel} returned an error: <code>{error}</code>
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
