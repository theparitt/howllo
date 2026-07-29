"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import {
  getMyInvitations,
  getNotifications,
  markAllNotificationsRead,
  markNotificationRead,
  respondToInvitation,
} from "@/lib/api";
import {
  getCurrentWorkspaceSlug,
  readStoredBearerToken,
  subscribeToBearerTokenChange,
} from "@/components/dev-auth-panel";
import { buildBoardWebSocketUrl } from "@/lib/realtime";
import type { MyInvitation, Notification } from "@/lib/types";

type Filter = "all" | "unread" | "invitations";

function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.round(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.round(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.round(hrs / 24)}d ago`;
}

// The signed-in home: a timeline of activity on the posts you follow, authored,
// or commented on — plus pending staff invitations pinned on top. Built from the
// user-scoped notification history (the same substrate as the bell + toasts).
export function ActivityFeed() {
  const router = useRouter();
  const [tenant, setTenant] = useState<string | null>(null);
  const [signedIn, setSignedIn] = useState(false);
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [invites, setInvites] = useState<MyInvitation[]>([]);
  const [filter, setFilter] = useState<Filter>("all");
  const [busyId, setBusyId] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    const resolve = () => setTenant(getCurrentWorkspaceSlug());
    resolve();
    return subscribeToBearerTokenChange(resolve);
  }, []);

  const load = useCallback(async (tenantSlug: string) => {
    const token = readStoredBearerToken(tenantSlug).trim();
    if (!token) {
      setSignedIn(false);
      setLoaded(true);
      return;
    }
    setSignedIn(true);
    try {
      const [items, inv] = await Promise.all([
        getNotifications(token),
        getMyInvitations(token).catch(() => [] as MyInvitation[]),
      ]);
      setNotifications(items);
      setInvites(inv);
    } catch {
      /* keep whatever we had */
    } finally {
      setLoaded(true);
    }
  }, []);

  const tokenRef = useRef<string>("");
  useEffect(() => {
    if (!tenant) return;
    tokenRef.current = readStoredBearerToken(tenant).trim();
    void load(tenant);
    if (!tokenRef.current) return;

    let socket: WebSocket | null = null;
    let reconnect: number | null = null;
    let destroyed = false;
    const connect = () => {
      const t = readStoredBearerToken(tenant).trim();
      if (!t) return;
      socket = new WebSocket(buildBoardWebSocketUrl({ tenantSlug: tenant, token: t }));
      socket.onmessage = () => void load(tenant);
      socket.onclose = () => {
        if (destroyed) return;
        reconnect = window.setTimeout(connect, 1500);
      };
      socket.onerror = () => socket?.close();
    };
    connect();
    return () => {
      destroyed = true;
      if (reconnect !== null) window.clearTimeout(reconnect);
      if (socket) {
        socket.onclose = null;
        socket.close();
      }
    };
  }, [tenant, load]);

  const unreadCount = useMemo(
    () => notifications.filter((n) => !n.is_read).length,
    [notifications],
  );

  const visible = useMemo(() => {
    if (filter === "unread") return notifications.filter((n) => !n.is_read);
    if (filter === "invitations")
      return notifications.filter((n) => n.event_type.startsWith("invitation"));
    return notifications;
  }, [filter, notifications]);

  const hrefFor = (n: Notification) => {
    const q = tenant ? `?tenant=${encodeURIComponent(tenant)}` : "";
    return n.post_id ? `/posts/${n.post_id}${q}` : `/dashboard${q}`;
  };

  const openNotification = async (n: Notification) => {
    const token = tenant ? readStoredBearerToken(tenant).trim() : "";
    if (token && !n.is_read) {
      try {
        await markNotificationRead(n.id, token);
        setNotifications((cur) => cur.map((x) => (x.id === n.id ? { ...x, is_read: true } : x)));
      } catch {
        /* ignore */
      }
    }
    router.push(hrefFor(n));
  };

  const readAll = async () => {
    const token = tenant ? readStoredBearerToken(tenant).trim() : "";
    if (!token) return;
    await markAllNotificationsRead(token).catch(() => {});
    setNotifications((cur) => cur.map((x) => ({ ...x, is_read: true })));
  };

  const respondInvite = async (invite: MyInvitation, action: "accept" | "reject") => {
    const token = tenant ? readStoredBearerToken(tenant).trim() : "";
    if (!token) return;
    setBusyId(invite.id);
    try {
      await respondToInvitation(invite.id, action, token);
      setInvites((cur) => cur.filter((i) => i.id !== invite.id));
      if (action === "accept") router.refresh();
    } catch {
      /* ignore */
    } finally {
      setBusyId(null);
    }
  };
  const skipInvite = (invite: MyInvitation) =>
    setInvites((cur) => cur.filter((i) => i.id !== invite.id));

  if (loaded && !signedIn) {
    return (
      <section className="panel empty-state">
        <h1 className="empty-state__title">Sign in to see your feed</h1>
        <p className="empty-state__copy">
          Once you sign into this workspace, updates on posts you follow, comment on, or author show up here.
        </p>
      </section>
    );
  }

  const showInvites = (filter === "all" || filter === "invitations") && invites.length > 0;

  return (
    <div className="page-stack">
      <section className="page-head">
        <h1 className="page-title">Your feed</h1>
        <p className="page-lead">Activity on the posts you follow, commented on, or authored.</p>
      </section>

      <div className="feed-filters">
        {(["all", "unread", "invitations"] as Filter[]).map((f) => (
          <button
            type="button"
            key={f}
            className={`feed-chip${filter === f ? " feed-chip--active" : ""}`}
            onClick={() => setFilter(f)}
          >
            {f === "all" ? "All" : f === "unread" ? `Unread${unreadCount ? ` · ${unreadCount}` : ""}` : "Invitations"}
          </button>
        ))}
        {unreadCount > 0 ? (
          <button type="button" className="text-button feed-readall" onClick={readAll}>
            Mark all read
          </button>
        ) : null}
      </div>

      {showInvites ? (
        <section className="panel invitations-panel">
          <div className="stack stack--tight">
            <h2 className="section-title">Invitations</h2>
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
                  <button type="button" className="button button--cta" disabled={busyId === invite.id} onClick={() => respondInvite(invite, "accept")}>
                    {busyId === invite.id ? "…" : "Accept"}
                  </button>
                  <button type="button" className="ghost-button" disabled={busyId === invite.id} onClick={() => respondInvite(invite, "reject")}>
                    Decline
                  </button>
                  <button type="button" className="text-button" disabled={busyId === invite.id} onClick={() => skipInvite(invite)}>
                    Skip
                  </button>
                </div>
              </div>
            ))}
          </div>
        </section>
      ) : null}

      <section className="panel">
        <div className="feed-list">
          {visible.length === 0 ? (
            <div className="empty-state">
              <h3 className="empty-state__title">
                {filter === "unread" ? "No unread activity" : filter === "invitations" ? "No invitations" : "Nothing here yet"}
              </h3>
              <p className="empty-state__copy">
                Follow a post or join a board — activity will show up in your feed.
              </p>
            </div>
          ) : (
            visible.map((n) => (
              <button
                type="button"
                key={n.id}
                className={`feed-item${n.is_read ? "" : " feed-item--unread"}`}
                onClick={() => openNotification(n)}
              >
                <div className="feed-item__main">
                  <span className="feed-item__title">{n.title}</span>
                  <span className="feed-item__body">{n.body}</span>
                </div>
                <span className="feed-item__time">{timeAgo(n.created_at)}</span>
              </button>
            ))
          )}
        </div>
      </section>
    </div>
  );
}
