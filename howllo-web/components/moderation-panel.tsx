"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import {
  getMyWorkspaceRole,
  moderateLock,
  moderateStatus,
  moderateVisibility,
  postOfficialResponse,
} from "@/lib/api";
import { readStoredBearerToken, subscribeToBearerTokenChange } from "@/components/dev-auth-panel";
import { StatusPill } from "@/components/status-pill";

// Allowed status transitions (mirrors the server's domain/status.rs).
const TRANSITIONS: Record<string, string[]> = {
  under_review: ["planned", "declined"],
  planned: ["in_progress", "declined"],
  in_progress: ["done", "planned"],
  done: ["in_progress"],
  declined: ["under_review"],
};

type Props = {
  tenantSlug: string;
  boardSlug: string;
  postId: string;
  currentStatus: string;
  isLocked: boolean;
};

// Moderation controls on the post page for owner/admin/moderator. Renders
// nothing for everyone else. Uses the same admin endpoints howllo-admin does.
export function ModerationPanel({ tenantSlug, boardSlug, postId, currentStatus, isLocked }: Props) {
  const router = useRouter();
  const [role, setRole] = useState<string | null>(null);
  const [reason, setReason] = useState("");
  const [official, setOfficial] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    const load = () => {
      const token = readStoredBearerToken(tenantSlug).trim();
      if (!token) {
        setRole(null);
        return;
      }
      getMyWorkspaceRole(tenantSlug, token).then(setRole).catch(() => setRole(null));
    };
    load();
    return subscribeToBearerTokenChange(load);
  }, [tenantSlug]);

  const canModerate = role === "owner" || role === "admin" || role === "moderator";
  if (!canModerate) return null;

  const token = () => readStoredBearerToken(tenantSlug).trim();
  const run = async (key: string, fn: () => Promise<void>, done?: () => void) => {
    setBusy(key);
    setError(null);
    setNotice(null);
    try {
      await fn();
      if (done) done();
      else router.refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Action failed.");
    } finally {
      setBusy(null);
    }
  };

  const nextStatuses = TRANSITIONS[currentStatus] ?? [];

  return (
    <section className="panel mod-panel">
      <div className="mod-panel__head">
        <h2 className="section-title" style={{ fontSize: "1.15rem" }}>Moderation</h2>
        <span className="mod-panel__role">{role}</span>
      </div>

      <div className="mod-block">
        <span className="mod-label">Status</span>
        <div className="mod-status-row">
          <StatusPill status={currentStatus} />
          {nextStatuses.length > 0 ? <span className="mod-arrow">→</span> : null}
          {nextStatuses.map((s) => (
            <button
              key={s}
              type="button"
              className="button button--small"
              disabled={busy !== null}
              onClick={() => run(`status-${s}`, () => moderateStatus(postId, s, reason, token()))}
            >
              {busy === `status-${s}` ? "…" : s.replace("_", " ")}
            </button>
          ))}
        </div>
        {nextStatuses.length > 0 ? (
          <input
            className="manage-input mod-reason"
            placeholder="Reason (optional)"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
          />
        ) : null}
      </div>

      <div className="mod-block">
        <span className="mod-label">Official response</span>
        <textarea
          className="manage-input"
          rows={2}
          placeholder="Reply on the record — followers get notified."
          value={official}
          onChange={(e) => setOfficial(e.target.value)}
        />
        <button
          type="button"
          className="button button--cta button--small"
          disabled={busy !== null || !official.trim()}
          onClick={() =>
            run("official", () => postOfficialResponse(postId, official.trim(), token()), () => {
              setOfficial("");
              setNotice("Official response posted.");
              router.refresh();
            })
          }
        >
          {busy === "official" ? "Posting…" : "Post official reply"}
        </button>
      </div>

      <div className="mod-block mod-block--row">
        <button
          type="button"
          className="ghost-button button--small"
          disabled={busy !== null}
          onClick={() => run("lock", () => moderateLock(postId, !isLocked, token()))}
        >
          {busy === "lock" ? "…" : isLocked ? "Unlock" : "Lock"}
        </button>
        <button
          type="button"
          className="ghost-button button--small button--danger"
          disabled={busy !== null}
          onClick={() => {
            if (!window.confirm("Hide this post from the public board?")) return;
            run("hide", () => moderateVisibility(postId, true, token()), () => {
              const q = tenantSlug ? `?tenant=${encodeURIComponent(tenantSlug)}` : "";
              router.push(`/boards/${boardSlug}${q}`);
            });
          }}
        >
          {busy === "hide" ? "…" : "Hide"}
        </button>
      </div>

      {notice ? <p className="success-text">{notice}</p> : null}
      {error ? <p className="error-text">{error}</p> : null}
    </section>
  );
}
