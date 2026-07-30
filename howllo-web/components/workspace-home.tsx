"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { createWorkspace, listMyWorkspaces } from "@/lib/api";
import { readAnyStoredBearerToken, subscribeToBearerTokenChange } from "@/components/dev-auth-panel";
import type { WorkspaceSummary } from "@/lib/types";

// The signed-in tenant's home: pick a workspace to manage, or create a new one.
// Shown at the app root (no workspace in the URL) instead of a "workspace
// required" error. Any stored workspace session works as the account credential.
export function WorkspaceHome() {
  const router = useRouter();
  const [token, setToken] = useState<string>("");
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const resolve = () => setToken(readAnyStoredBearerToken().trim());
    resolve();
    return subscribeToBearerTokenChange(resolve);
  }, []);

  const load = useCallback(async (t: string) => {
    try {
      setWorkspaces(await listMyWorkspaces(t));
    } catch {
      /* ignore */
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => {
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
      router.push(`/${encodeURIComponent(ws.slug)}`);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create workspace");
    } finally {
      setBusy(false);
    }
  };

  // Signed out: guide them to sign into a workspace they already have.
  if (loaded && !token) {
    return (
      <div className="page-stack" style={{ maxWidth: "42rem", margin: "0 auto" }}>
        <section className="panel empty-state">
          <span className="kicker">Welcome</span>
          <h1 className="empty-state__title">Sign in to get started</h1>
          <p className="empty-state__copy">
            Open your workspace to sign in, then come back here to manage it or spin up another —
            e.g. <code>/your-workspace/dashboard</code>.
          </p>
        </section>
      </div>
    );
  }

  return (
    <div className="page-stack" style={{ maxWidth: "48rem", margin: "0 auto" }}>
      <section className="page-head">
        <h1 className="page-title">Your workspaces</h1>
        <p className="page-lead">Open one to manage its board, or create a new one.</p>
      </section>

      <section className="ws-grid">
        {workspaces.map((ws) => (
          <Link key={ws.id} href={`/${encodeURIComponent(ws.slug)}`} className="ws-card">
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
        {error ? <p className="error-text">{error}</p> : null}
      </section>
    </div>
  );
}
