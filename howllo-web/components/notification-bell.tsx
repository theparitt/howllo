"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useRouter } from "next/navigation";
import {
  getNotifications,
  getUnreadCount,
  markAllNotificationsRead,
  markNotificationRead,
} from "@/lib/api";
import {
  getCurrentWorkspaceSlug,
  readStoredBearerToken,
  subscribeToBearerTokenChange,
} from "@/components/dev-auth-panel";
import { buildBoardWebSocketUrl } from "@/lib/realtime";
import type { Notification } from "@/lib/types";

const POLL_MS = 30_000;
const TOAST_MS = 5_000;
const MAX_TOASTS = 3;

function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.round(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.round(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.round(hrs / 24)}d ago`;
}

// One self-contained island: the header bell + unread badge + dropdown center,
// and a fixed bottom-right toast stack. Uses the existing per-workspace realtime
// stream (plus a slow poll) to refetch the user-scoped notification list and
// surface anything new as a toast. Renders nothing until signed in.
export function NotificationBell() {
  const router = useRouter();
  const [tenant, setTenant] = useState<string | null>(null);
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [unread, setUnread] = useState(0);
  const [open, setOpen] = useState(false);
  const [toasts, setToasts] = useState<Notification[]>([]);

  const knownIds = useRef<Set<string>>(new Set());
  const primed = useRef(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const bellRef = useRef<HTMLButtonElement>(null);
  const centerRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; right: number }>({ top: 64, right: 24 });

  const toggleOpen = () => {
    setOpen((v) => {
      const next = !v;
      if (next && bellRef.current) {
        const r = bellRef.current.getBoundingClientRect();
        setPos({ top: r.bottom + 8, right: Math.max(12, window.innerWidth - r.right) });
      }
      return next;
    });
  };

  const dismissToast = useCallback((id: string) => {
    setToasts((current) => current.filter((t) => t.id !== id));
  }, []);

  const load = useCallback(async (tenantSlug: string) => {
    const token = readStoredBearerToken(tenantSlug).trim();
    if (!token) return;
    try {
      const [items, count] = await Promise.all([
        getNotifications(token),
        getUnreadCount(token),
      ]);
      // New, unread items that we haven't seen become toasts (but never on the
      // very first load, or we'd toast the whole backlog).
      if (primed.current) {
        const fresh = items.filter((n) => !knownIds.current.has(n.id) && !n.is_read);
        if (fresh.length > 0) {
          setToasts((current) => [...fresh.slice(0, MAX_TOASTS), ...current].slice(0, MAX_TOASTS + 2));
        }
      }
      items.forEach((n) => knownIds.current.add(n.id));
      primed.current = true;
      setNotifications(items);
      setUnread(count);
    } catch {
      /* non-critical surface — stay quiet */
    }
  }, []);

  // Resolve tenant + token on mount and whenever auth changes.
  useEffect(() => {
    const resolve = () => setTenant(getCurrentWorkspaceSlug());
    resolve();
    const unsub = subscribeToBearerTokenChange(resolve);
    return unsub;
  }, []);

  // Load + realtime + poll, scoped to the active workspace.
  useEffect(() => {
    if (!tenant) return;
    const token = readStoredBearerToken(tenant).trim();
    if (!token) return;

    primed.current = false;
    knownIds.current = new Set();
    void load(tenant);

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

    const poll = window.setInterval(() => void load(tenant), POLL_MS);

    return () => {
      destroyed = true;
      window.clearInterval(poll);
      if (reconnect !== null) window.clearTimeout(reconnect);
      if (socket) {
        socket.onclose = null;
        socket.close();
      }
    };
  }, [tenant, load]);

  // Auto-dismiss toasts.
  useEffect(() => {
    if (toasts.length === 0) return;
    const timers = toasts.map((t) =>
      window.setTimeout(() => dismissToast(t.id), TOAST_MS),
    );
    return () => timers.forEach((id) => window.clearTimeout(id));
  }, [toasts, dismissToast]);

  // Close the dropdown on outside click (the center is portaled, so check both).
  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      const target = e.target as Node;
      const inBell = rootRef.current?.contains(target);
      const inCenter = centerRef.current?.contains(target);
      if (!inBell && !inCenter) setOpen(false);
    };
    document.addEventListener("mousedown", onClick);
    return () => document.removeEventListener("mousedown", onClick);
  }, [open]);

  const hrefFor = (n: Notification) => {
    const q = tenant ? `?tenant=${encodeURIComponent(tenant)}` : "";
    if (n.post_id) return `/posts/${n.post_id}${q}`;
    return tenant ? `/${encodeURIComponent(tenant)}` : "/";
  };

  const openNotification = async (n: Notification) => {
    const token = tenant ? readStoredBearerToken(tenant).trim() : "";
    if (token && !n.is_read) {
      try {
        await markNotificationRead(n.id, token);
        setUnread((u) => Math.max(0, u - 1));
        setNotifications((cur) => cur.map((x) => (x.id === n.id ? { ...x, is_read: true } : x)));
      } catch {
        /* ignore */
      }
    }
    setOpen(false);
    router.push(hrefFor(n));
  };

  const readAll = async () => {
    const token = tenant ? readStoredBearerToken(tenant).trim() : "";
    if (!token) return;
    try {
      await markAllNotificationsRead(token);
      setUnread(0);
      setNotifications((cur) => cur.map((x) => ({ ...x, is_read: true })));
    } catch {
      /* ignore */
    }
  };

  if (!tenant) return null;
  const hasToken = readStoredBearerToken(tenant).trim().length > 0;
  if (!hasToken) return null;

  return (
    <div className="notif" ref={rootRef}>
      <button
        ref={bellRef}
        type="button"
        className="notif__bell"
        aria-label={`Notifications${unread ? `, ${unread} unread` : ""}`}
        aria-expanded={open}
        onClick={toggleOpen}
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
          <path d="M13.73 21a2 2 0 0 1-3.46 0" />
        </svg>
        {unread > 0 ? <span className="notif__badge">{unread > 9 ? "9+" : unread}</span> : null}
      </button>

      {open && typeof document !== "undefined"
        ? createPortal(
        <div
          className="notif__center"
          role="dialog"
          aria-label="Notifications"
          ref={centerRef}
          style={{ position: "fixed", top: pos.top, right: pos.right }}
        >
          <div className="notif__center-head">
            <strong>Notifications</strong>
            {unread > 0 ? (
              <button type="button" className="text-button" onClick={readAll}>
                Mark all read
              </button>
            ) : null}
          </div>
          <div className="notif__list">
            {notifications.length === 0 ? (
              <div className="notif__empty">You're all caught up.</div>
            ) : (
              notifications.slice(0, 20).map((n) => (
                <button
                  type="button"
                  key={n.id}
                  className={`notif__item${n.is_read ? "" : " notif__item--unread"}`}
                  onClick={() => openNotification(n)}
                >
                  <span className="notif__item-title">{n.title}</span>
                  <span className="notif__item-body">{n.body}</span>
                  <span className="notif__item-time">{timeAgo(n.created_at)}</span>
                </button>
              ))
            )}
          </div>
        </div>,
            document.body,
          )
        : null}

      {typeof document !== "undefined"
        ? createPortal(
            <div className="toast-stack" aria-live="polite">
              {toasts.slice(0, MAX_TOASTS).map((t) => (
                <button
                  type="button"
                  key={t.id}
                  className="toast"
                  onClick={() => {
                    dismissToast(t.id);
                    void openNotification(t);
                  }}
                >
                  <span className="toast__title">{t.title}</span>
                  <span className="toast__body">{t.body}</span>
                  <span
                    className="toast__close"
                    aria-hidden="true"
                    onClick={(e) => {
                      e.stopPropagation();
                      dismissToast(t.id);
                    }}
                  >
                    ×
                  </span>
                </button>
              ))}
              {toasts.length > MAX_TOASTS ? (
                <div className="toast toast--more">+{toasts.length - MAX_TOASTS} more</div>
              ) : null}
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}
