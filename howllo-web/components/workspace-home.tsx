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
  const [creating, setCreating] = useState(false);
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
    if (busy || !name.trim()) return;
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
          {staffSignIn ? <p className="section-subtitle">Sign in with RooIAM.</p> : null}
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
    <div className="workspace-home">
      <header className="workspace-home__header">
        <div>
          <h1 className="page-title">Your workspaces</h1>
          <p className="workspace-home__intro">Choose a workspace to manage its boards and settings.</p>
        </div>
        {loaded && workspaces.length > 0 ? (
          <button className="button workspace-home__new" type="button" onClick={() => setCreating((value) => !value)} aria-expanded={creating} aria-controls="workspace-create">
            {creating ? "Cancel" : "+ New workspace"}
          </button>
        ) : null}
      </header>

      {error ? <p className="error-text" role="alert">{error}</p> : null}

      {!loaded ? <p className="workspace-home__loading">Loading workspaces…</p> : null}

      {loaded && workspaces.length > 0 ? <section className="workspace-home__list" aria-label="Your workspaces">
        {workspaces.map((ws) => (
          <Link key={ws.id} href={staffSignIn ? `/app/${encodeURIComponent(ws.slug)}` : publicWebOrigin ? `${publicWebOrigin.replace(/\/$/, "")}/${encodeURIComponent(ws.slug)}` : `/${encodeURIComponent(ws.slug)}`} className="workspace-home__item" aria-disabled={busy} onClick={(event) => {
            if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
            event.preventDefault();
            if (!busy) void openWorkspace(ws.slug);
          }}>
            <span className="workspace-home__avatar" aria-hidden="true">{ws.name.trim().charAt(0).toUpperCase() || "W"}</span>
            <span className="workspace-home__details">
              <strong>{ws.name}</strong>
              <span>/{ws.slug} <span aria-hidden="true">·</span> {ws.board_count} {ws.board_count === 1 ? "board" : "boards"} <span aria-hidden="true">·</span> {ws.member_count} {ws.member_count === 1 ? "member" : "members"}</span>
            </span>
            <span className={`workspace-home__status ${ws.is_published ? "workspace-home__status--live" : ""}`}>{ws.is_published ? "Published" : "Private"}</span>
            <span className="workspace-home__arrow" aria-hidden="true">→</span>
          </Link>
        ))}
      </section> : null}

      {loaded && (creating || workspaces.length === 0) ? <section className="workspace-home__create" id="workspace-create">
        <h2>{workspaces.length === 0 ? "Create your first workspace" : "New workspace"}</h2>
        <p>A workspace keeps one product&rsquo;s boards and team together.</p>
        <form className="workspace-home__form" onSubmit={(event) => { event.preventDefault(); void create(); }}>
          <label htmlFor="workspace-name">Workspace name</label>
          <input
            id="workspace-name"
            className="manage-input"
            placeholder="e.g. My product"
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={120}
            autoFocus={creating}
          />
          <span className="workspace-home__hint">Starts private with no boards. You can set it up before publishing.</span>
          <button type="submit" className="button button--cta" disabled={busy || !name.trim()}>
            {busy ? "Creating…" : "Create workspace"}
          </button>
        </form>
      </section> : null}
    </div>
  );
}
