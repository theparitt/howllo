"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { getMyInvitations, respondToInvitation } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import type { MyInvitation } from "@/lib/types";

// Invitee-facing surface: shows staff invitations the signed-in user can
// accept / decline / skip. Renders nothing when there are none (or when the
// user isn't signed into this workspace). Accepting refreshes so the new access
// takes effect immediately.
export function PendingInvitations({ tenantSlug }: { tenantSlug: string }) {
  const router = useRouter();
  const [invites, setInvites] = useState<MyInvitation[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const token = readStoredBearerToken(tenantSlug);
    if (!token) return;
    let cancelled = false;
    getMyInvitations(token)
      .then((items) => {
        if (!cancelled) setInvites(items);
      })
      .catch(() => {
        /* silent — invitations are a non-critical surface */
      });
    return () => {
      cancelled = true;
    };
  }, [tenantSlug]);

  if (invites.length === 0) return null;

  const respond = async (invite: MyInvitation, action: "accept" | "reject") => {
    const token = readStoredBearerToken(tenantSlug);
    if (!token) return;
    setBusyId(invite.id);
    setError(null);
    try {
      await respondToInvitation(invite.id, action, token);
      setInvites((current) => current.filter((item) => item.id !== invite.id));
      if (action === "accept") router.refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not respond to the invitation.");
    } finally {
      setBusyId(null);
    }
  };

  // Skip = dismiss locally for this session only (invite stays pending server-side).
  const skip = (invite: MyInvitation) =>
    setInvites((current) => current.filter((item) => item.id !== invite.id));

  return (
    <section className="panel invitations-panel">
      <div className="stack stack--tight">
        <h2 className="section-title">Invitations</h2>
        <p className="section-subtitle">You've been invited to join a workspace team.</p>
      </div>
      <div className="list-stack" style={{ marginTop: "1rem" }}>
        {invites.map((invite) => (
          <div className="invitation-card" key={invite.id}>
            <div className="invitation-card__body">
              <strong>{invite.tenant_name}</strong>
              <p className="section-subtitle">
                Join as <span className="chip" data-status="under_review">{invite.role}</span>
                {invite.invited_by_name ? ` · invited by ${invite.invited_by_name}` : ""}
              </p>
            </div>
            <div className="invitation-card__actions">
              <button
                type="button"
                className="button button--cta"
                disabled={busyId === invite.id}
                onClick={() => respond(invite, "accept")}
              >
                {busyId === invite.id ? "…" : "Accept"}
              </button>
              <button
                type="button"
                className="ghost-button"
                disabled={busyId === invite.id}
                onClick={() => respond(invite, "reject")}
              >
                Decline
              </button>
              <button
                type="button"
                className="text-button"
                disabled={busyId === invite.id}
                onClick={() => skip(invite)}
              >
                Skip
              </button>
            </div>
          </div>
        ))}
      </div>
      {error ? <p className="error-text" style={{ marginTop: "0.75rem" }}>{error}</p> : null}
    </section>
  );
}
