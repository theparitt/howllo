"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { ApiError, createWorkspace, createWorkspaceSession, getMyWorkspaceRole, listMyWorkspaces } from "@/lib/api";
import { persistBearerToken, readAccountToken, readAnyStoredBearerToken, readStoredBearerToken, subscribeToBearerTokenChange, writeAccountToken } from "@/components/dev-auth-panel";
import { requestLogin } from "@/components/auth-login";
import { RooiamInlineLogin } from "@/components/rooiam-inline-login";
import { ENABLED_AUTH_PROVIDERS } from "@/lib/auth-provider";
import type { WorkspaceSummary } from "@/lib/types";

// The signed-in tenant's home: pick a workspace to manage, or create a new one.
// Shown at the app root (no workspace in the URL) instead of a "workspace
// required" error. Any stored workspace session works as the account credential.
export function WorkspaceHome({ publicWebOrigin, staffSignIn = false }: { publicWebOrigin?: string; staffSignIn?: boolean } = {}) {
  const router = useRouter();
  const [token, setToken] = useState<string>("");
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openWorkspace = async (slug: string) => {
    setBusy(true);
    setError(null);
    try {
      const accountToken = readAccountToken().trim();
      let workspaceToken = readStoredBearerToken(slug).trim();
      if (!workspaceToken && accountToken) {
        const session = await createWorkspaceSession({
          tenantSlug: slug,
          accessToken: accountToken.replace(/^Bearer\s+/i, ""),
        });
        workspaceToken = `Bearer ${session.session_token}`;
        persistBearerToken(slug, workspaceToken);
      }
      const role = workspaceToken ? await getMyWorkspaceRole(slug, workspaceToken).catch(() => null) : null;
      if (role === "owner" || role === "admin" || role === "moderator") {
        router.push(`/app/${encodeURIComponent(slug)}`);
      } else if (publicWebOrigin) {
        window.location.assign(`${publicWebOrigin.replace(/\/$/, "")}/${encodeURIComponent(slug)}`);
      } else {
        router.push(`/${encodeURIComponent(slug)}`);
      }
    } catch (cause) {
      if (cause instanceof ApiError && cause.status === 401) {
        writeAccountToken("");
        setError("Your sign-in has expired. Please sign in again.");
      } else {
        setError(cause instanceof Error ? cause.message : "Could not open workspace");
      }
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    const resolve = () => setToken(readAnyStoredBearerToken().trim());
    resolve();
    return subscribeToBearerTokenChange(resolve);
  }, []);

  const load = useCallback(async (t: string) => {
    const isCurrent = () => readAnyStoredBearerToken().trim() === t;
    try {
      const result = await listMyWorkspaces(t);
      if (isCurrent()) setWorkspaces(result);
    } catch (cause) {
      if (!isCurrent()) return;
      if (cause instanceof ApiError && cause.status === 401) {
        writeAccountToken("");
        setError("Your sign-in has expired. Please sign in again.");
      } else {
        setError(cause instanceof Error ? cause.message : "Could not load workspaces");
      }
    } finally {
      if (isCurrent()) setLoaded(true);
    }
  }, []);

  useEffect(() => {
    setWorkspaces([]);
    if (!token) {
      setLoaded(true);
      return;
    }
    setLoaded(false);
    void load(token);
  }, [token, load]);

  const create = async () => {
    setBusy(true);
    setError(null);
    try {
      const ws = await createWorkspace(name.trim(), token);
      setWorkspaces((current) => [...current, ws]);
      await openWorkspace(ws.slug);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create workspace");
    } finally {
      setBusy(false);
    }
  };

  // Signed out: guide them to sign into a workspace they already have.
  if (loaded && !token) {
    const rooiamOnly = ENABLED_AUTH_PROVIDERS.length === 1 && ENABLED_AUTH_PROVIDERS[0] === "rooiam";
    if (rooiamOnly) {
      return (
        <section className="rooiam-inline-login" aria-label={staffSignIn ? "Workspace staff sign in" : "Sign in to Howllo"}>
          <h1 className="rooiam-inline-login__title">{staffSignIn ? "Workspace staff sign in" : "Sign in to Howllo"}</h1>
          {staffSignIn ? <p className="section-subtitle">For workspace owners, admins and moderators. Continue with your RooIAM account.</p> : null}
          <RooiamInlineLogin />
        </section>
      );
    }
    return (
      <div className="page-stack" style={{ maxWidth: "42rem", margin: "0 auto" }}>
        <section className="panel empty-state">
          <span className="kicker">Welcome to Howllo</span>
          <h1 className="empty-state__title">Sign in to get started</h1>
          <p className="empty-state__copy">
            Sign in to create your feedback board and invite your team.
          </p>
          <div className="hero__actions" style={{ marginTop: "1rem", justifyContent: "center" }}>
            <button className="button button--cta" type="button" onClick={requestLogin}>
              Sign in
            </button>
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="page-stack" style={{ maxWidth: "48rem", margin: "0 auto" }}>
      <section className="page-head">
        <h1 className="page-title">Your workspaces</h1>
        <p className="page-lead">Open a workspace to manage its boards, or create a new one.</p>
      </section>

      <section className="ws-grid">
        {workspaces.map((ws) => (
          <Link key={ws.id} href={publicWebOrigin ? `${publicWebOrigin.replace(/\/$/, "")}/${encodeURIComponent(ws.slug)}` : `/${encodeURIComponent(ws.slug)}`} className="ws-card" aria-disabled={busy} onClick={(event) => {
            if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
            event.preventDefault();
            if (!busy) void openWorkspace(ws.slug);
          }}>
            <strong>{ws.name}</strong>
            <span className="section-subtitle">
              /{ws.slug} · {ws.board_count} boards · {ws.member_count} members
            </span>
          </Link>
        ))}
        {loaded && workspaces.length === 0 ? (
          <p className="section-subtitle">You don&rsquo;t have any workspaces yet — create your first one below.</p>
        ) : null}
      </section>

      {error ? <p className="error-text" role="alert">{error}</p> : null}

      <section className="panel">
        <h2 className="section-title">Create a workspace</h2>
        <div className="manage-form" style={{ marginTop: "1rem" }}>
          <input
            className="manage-input"
            placeholder="My product"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && name.trim() && !busy) create();
            }}
          />
          <button type="button" className="button button--cta" disabled={busy || !name.trim()} onClick={create}>
            {busy ? "Creating…" : "Create workspace"}
          </button>
        </div>
        <p className="section-subtitle" style={{ marginTop: "0.6rem" }}>
          You&rsquo;ll get a slug and starter boards, and become its owner.
        </p>
      </section>
    </div>
  );
}
