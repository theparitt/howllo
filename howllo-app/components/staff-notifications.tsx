"use client";

import { useCallback, useEffect, useState } from "react";
import { getNotifications, getUnreadCount, markAllNotificationsRead } from "@/lib/api";
import { readStoredBearerToken, subscribeToBearerTokenChange } from "@/components/dev-auth-panel";
import type { Notification } from "@/lib/types";

export function StaffNotifications({ tenantSlug }: { tenantSlug: string }) {
  const [token, setToken] = useState("");
  const [items, setItems] = useState<Notification[]>([]);
  const [unread, setUnread] = useState(0);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const sync = () => setToken(readStoredBearerToken(tenantSlug).trim());
    sync();
    return subscribeToBearerTokenChange(sync);
  }, [tenantSlug]);

  const refresh = useCallback(async () => {
    if (!token) return;
    try {
      const [nextItems, nextUnread] = await Promise.all([
        getNotifications(token), getUnreadCount(token),
      ]);
      if (readStoredBearerToken(tenantSlug).trim() !== token) return;
      setItems(nextItems);
      setUnread(nextUnread);
    } catch {
      // Notifications should never block workspace management.
    }
  }, [tenantSlug, token]);

  useEffect(() => {
    if (!token) { setItems([]); setUnread(0); return; }
    void refresh();
    const timer = window.setInterval(() => void refresh(), 15_000);
    window.addEventListener("focus", refresh);
    return () => { window.clearInterval(timer); window.removeEventListener("focus", refresh); };
  }, [token, refresh]);

  if (!token) return null;

  const readAll = async () => {
    try {
      await markAllNotificationsRead(token);
      setItems((current) => current.map((item) => ({ ...item, is_read: true })));
      setUnread(0);
    } catch {
      void refresh();
    }
  };

  return <div className="staff-notifications">
    <button type="button" className="staff-notifications__trigger" aria-label={`Notifications${unread ? `, ${unread} unread` : ""}`} aria-expanded={open} onClick={() => { setOpen((value) => !value); void refresh(); }}>
      <svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M10 21h4"/></svg>
      {unread > 0 ? <span className="staff-notifications__count">{unread > 9 ? "9+" : unread}</span> : null}
    </button>
    {open ? <div className="staff-notifications__panel" role="dialog" aria-label="Workspace notifications">
      <div className="staff-notifications__head"><strong>Notifications</strong>{unread > 0 ? <button type="button" onClick={() => void readAll()}>Mark all read</button> : null}</div>
      {items.length ? <div className="staff-notifications__list">{items.slice(0, 12).map((item) => <div key={item.id} className={`staff-notifications__item${item.is_read ? "" : " staff-notifications__item--unread"}`}>
        <strong>{item.title}</strong><span>{item.body}</span><small>{new Date(item.created_at).toLocaleString()}</small>
      </div>)}</div> : <p className="staff-notifications__empty">No notifications yet.</p>}
    </div> : null}
  </div>;
}
