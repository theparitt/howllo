import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { admin } from "@howllo/api-client";
import { adminRoutes } from "@howllo/config";
import type { AdminTenantSummary } from "@howllo/types";
import { useSession } from "../lib/session";

// Sidebar workspace switcher. Shows the active workspace and opens a dropdown
// to switch between workspaces in one click (replacing the type-a-slug field).

export function WorkspaceSwitcher() {
  const { client, tenant, authorization, setTenant } = useSession();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<AdminTenantSummary[]>([]);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!authorization) return;
    let cancelled = false;
    admin(client)
      .listTenants()
      .then((data) => {
        if (!cancelled) setItems(data);
      })
      .catch(() => {
        /* leave empty; user can still manage from Workspaces */
      });
    return () => {
      cancelled = true;
    };
  }, [client, authorization, tenant]);

  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onClick);
    return () => document.removeEventListener("mousedown", onClick);
  }, [open]);

  const active = items.find((t) => t.slug === tenant);
  const label = active?.name ?? tenant ?? "Select workspace";
  const initial = (label[0] ?? "?").toUpperCase();

  const choose = (slug: string) => {
    setTenant(slug);
    setOpen(false);
  };

  return (
    <div className="ws-switcher" ref={ref}>
      <button
        className="ws-trigger"
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <span className="ws-avatar">{initial}</span>
        <span className="ws-trigger-text">
          <span className="ws-trigger-label">Workspace</span>
          <strong>{label}</strong>
        </span>
        <svg className="ws-chevron" width="14" height="14" viewBox="0 0 24 24" aria-hidden>
          <path
            d="M6 9l6 6 6-6"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>

      {open ? (
        <div className="ws-menu" role="menu">
          <div className="ws-menu-head">Switch workspace</div>
          <div className="ws-menu-list">
            {items.length === 0 ? (
              <div className="ws-menu-empty">No workspaces yet.</div>
            ) : (
              items.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  role="menuitem"
                  className={
                    t.slug === tenant ? "ws-item ws-item--active" : "ws-item"
                  }
                  onClick={() => choose(t.slug)}
                >
                  <span className="ws-avatar ws-avatar--sm">
                    {(t.name[0] ?? "?").toUpperCase()}
                  </span>
                  <span className="ws-item-text">
                    <strong>{t.name}</strong>
                    <span>
                      {t.board_count} boards - {t.member_count} members
                    </span>
                  </span>
                  {t.slug === tenant ? <span className="ws-check">OK</span> : null}
                </button>
              ))
            )}
          </div>
          <button
            type="button"
            className="ws-manage"
            onClick={() => {
              setOpen(false);
              navigate(adminRoutes.tenants());
            }}
          >
            Manage workspaces
          </button>
        </div>
      ) : null}
    </div>
  );
}
