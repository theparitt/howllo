import { NextRequest, NextResponse } from "next/server";
import { getWorkspaceAuthConfig } from "@/lib/api";

export const dynamic = "force-dynamic";

type TokenRequest = {
  code?: string;
  codeVerifier?: string;
  redirectUri?: string;
  tenantSlug?: string | null;
};

function error(message: string, status: number) {
  return NextResponse.json({ error_description: message }, { status, headers: { "cache-control": "no-store" } });
}

export async function POST(request: NextRequest) {
  let body: TokenRequest;
  try {
    body = await request.json();
  } catch {
    return error("Invalid token request.", 400);
  }

  const code = body.code?.trim();
  const verifier = body.codeVerifier?.trim();
  const redirectUri = body.redirectUri?.trim();
  if (!code || !verifier || !redirectUri || code.length > 4096 || verifier.length > 256) {
    return error("Missing or invalid OIDC code or verifier.", 400);
  }

  let callback: URL;
  try {
    callback = new URL(redirectUri);
  } catch {
    return error("Invalid callback URL.", 400);
  }
  const requestOrigin = new URL(request.url).origin;
  if (callback.pathname !== "/auth/callback" || callback.search || callback.hash ||
      callback.origin !== requestOrigin || request.headers.get("origin") !== requestOrigin) {
    return error("Callback origin does not match this app.", 400);
  }

  const globalBase = process.env.NEXT_PUBLIC_ROOIAM_WIDGET_BASE_URL?.trim();
  const globalClientId = process.env.NEXT_PUBLIC_ROOIAM_WIDGET_CLIENT_ID?.trim();
  if (!globalBase || !globalClientId) return error("RooIAM is not configured.", 503);

  let baseUrl = globalBase;
  let clientId = globalClientId;
  if (body.tenantSlug) {
    if (!/^[a-z0-9][a-z0-9-]{0,99}$/.test(body.tenantSlug)) {
      return error("Invalid workspace.", 400);
    }
    try {
      const config = await getWorkspaceAuthConfig(body.tenantSlug);
      if (config.provider !== "rooiam") return error("Workspace uses another sign-in provider.", 400);
      baseUrl = config.rooiam_widget_base_url?.trim() || globalBase;
      clientId = config.rooiam_client_id?.trim() || globalClientId;
    } catch {
      return error("Could not load workspace sign-in settings.", 502);
    }
  }

  let provider: URL;
  let allowed: URL;
  try {
    provider = new URL(baseUrl);
    allowed = new URL(globalBase);
  } catch {
    return error("Invalid RooIAM URL.", 503);
  }
  // Workspace settings may choose a client on the configured RooIAM service,
  // but cannot turn this server-side exchange into a request to another host.
  if (provider.origin !== allowed.origin ||
      (provider.protocol !== "https:" && !["localhost", "127.0.0.1"].includes(provider.hostname))) {
    return error("Workspace RooIAM origin is not allowed.", 400);
  }

  const form = new URLSearchParams({
    grant_type: "authorization_code",
    code,
    redirect_uri: redirectUri,
    client_id: clientId,
    code_verifier: verifier,
  });
  try {
    const response = await fetch(new URL("/v1/oidc/token", provider.origin), {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body: form,
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(10000),
    });
    const payload = await response.json().catch(() => ({}));
    if (!response.ok || typeof payload.access_token !== "string") {
      return error(
        typeof payload.error_description === "string"
          ? payload.error_description.slice(0, 300)
          : "RooIAM could not exchange the authorization code.",
        502,
      );
    }
    return NextResponse.json(
      { access_token: payload.access_token },
      { headers: { "cache-control": "no-store" } },
    );
  } catch {
    return error("Could not reach RooIAM token service.", 502);
  }
}
