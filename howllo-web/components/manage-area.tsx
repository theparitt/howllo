"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { RooiamInlineLogin } from "@/components/rooiam-inline-login";
import { ENABLED_AUTH_PROVIDERS } from "@/lib/auth-provider";
import {
  disableSso,
  getMyWorkspaceRole,
  getSsoConfig,
  regenerateSsoSecret,
  type SsoConfig,
  managerCreateInvitation,
  managerCreateBoard,
  managerListBoards,
  managerListInvitations,
  managerListMembers,
  managerRemoveMember,
  managerUpdateBoard,
  managerUpdateMemberRole,
  managerWithdrawInvitation,
} from "@/lib/api";
import {
  getCurrentWorkspaceSlug,
  readStoredBearerToken,
  subscribeToBearerTokenChange,
} from "@/components/dev-auth-panel";
import type { DashboardSection, ManageBoard, MyInvitation, WorkspaceMember } from "@/lib/types";
import { API_BASE_URL } from "@/lib/config";

const INVITE_ROLES = ["admin", "moderator", "member"];
const MEMBER_ROLES = ["owner", "admin", "moderator", "member"];
const BOARD_TYPES = ["feature-requests", "bug-reports", "discussions", "announcements"];
const SECTIONS: { id: DashboardSection; label: string }[] = [
  { id: "progress", label: "Progress" },
  { id: "latest", label: "Latest" },
  { id: "top", label: "Top requests" },
];

function statusTone(status: string): string {
  if (status === "accepted") return "green";
  if (status === "pending") return "blue";
  return "warm";
}

export function ManageArea() {
  const [tenant, setTenant] = useState<string | null>(null);
  const [role, setRole] = useState<string | null | undefined>(undefined);
  const [tab, setTab] = useState<"team" | "boards" | "signin">("boards");

  useEffect(() => {
    const resolve = () => setTenant(getCurrentWorkspaceSlug());
    resolve();
    return subscribeToBearerTokenChange(resolve);
  }, []);

  useEffect(() => {
    if (!tenant) return;
    const token = readStoredBearerToken(tenant).trim();
    if (!token) {
      setRole(null);
      return;
    }
    getMyWorkspaceRole(tenant, token).then(setRole).catch(() => setRole(null));
  }, [tenant]);

  if (role === undefined) {
    return <section className="panel"><p className="section-subtitle">Loading…</p></section>;
  }

  const canManage = role === "owner" || role === "admin";
  if (!canManage) {
    if (tenant && !readStoredBearerToken(tenant).trim() && ENABLED_AUTH_PROVIDERS.length === 1 && ENABLED_AUTH_PROVIDERS[0] === "rooiam") {
      return <section className="rooiam-inline-login" aria-label="Sign in to manage workspace">
        <h1 className="rooiam-inline-login__title">Sign in to Howllo App</h1>
        <p className="section-subtitle">Workspace owners and admins can manage boards here.</p>
        <RooiamInlineLogin tenantSlug={tenant} />
      </section>;
    }
    return (
      <section className="panel empty-state">
        <h1 className="empty-state__title">Manager access only</h1>
        <p className="empty-state__copy">
          {tenant && readStoredBearerToken(tenant).trim()
            ? "You need an owner or admin role in this workspace to manage it."
            : "Sign into this workspace to manage its boards and team."}
        </p>
      </section>
    );
  }

  return (
    <div className="page-stack">
      <section className="page-head">
        <h1 className="page-title">Manage workspace</h1>
        <p className="page-lead">Create boards, choose who can see them, and share public board links with your users.</p>
      </section>

      <div className="feed-filters">
        <button type="button" className={`feed-chip${tab === "team" ? " feed-chip--active" : ""}`} onClick={() => setTab("team")}>
          Team
        </button>
        <button type="button" className={`feed-chip${tab === "boards" ? " feed-chip--active" : ""}`} onClick={() => setTab("boards")}>
          Boards
        </button>
        <button type="button" className={`feed-chip${tab === "signin" ? " feed-chip--active" : ""}`} onClick={() => setTab("signin")}>
          Sign-in (SSO)
        </button>
      </div>

      {tenant ? (
        tab === "team" ? (
          <TeamTab tenant={tenant} myRole={role!} />
        ) : tab === "boards" ? (
          <BoardsTab tenant={tenant} />
        ) : (
          <SsoTab tenant={tenant} />
        )
      ) : null}
    </div>
  );
}

function TeamTab({ tenant, myRole }: { tenant: string; myRole: string }) {
  const [invites, setInvites] = useState<MyInvitation[]>([]);
  const [members, setMembers] = useState<WorkspaceMember[]>([]);
  const [email, setEmail] = useState("");
  const [inviteRole, setInviteRole] = useState("moderator");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const token = () => readStoredBearerToken(tenant).trim();

  const reload = useCallback(async () => {
    const t = token();
    if (!t) return;
    try {
      const [inv, mem] = await Promise.all([
        managerListInvitations(tenant, t),
        managerListMembers(tenant, t),
      ]);
      setInvites(inv);
      setMembers(mem);
    } catch {
      /* ignore */
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tenant]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const invite = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await managerCreateInvitation(tenant, { email: email.trim(), role: inviteRole }, token());
      setNotice(`Invitation sent to ${email.trim()}.`);
      setEmail("");
      setInviteRole("moderator");
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to send invitation");
    } finally {
      setBusy(false);
    }
  };

  const ownerCount = members.filter((m) => m.role === "owner").length;

  return (
    <>
      <section className="panel">
        <h2 className="section-title">Invite a teammate</h2>
        <div className="manage-form" style={{ marginTop: "1rem" }}>
          <input className="manage-input" placeholder="person@example.com" value={email} onChange={(e) => setEmail(e.target.value)} />
          <select className="manage-input" value={inviteRole} onChange={(e) => setInviteRole(e.target.value)}>
            {INVITE_ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
          </select>
          <button type="button" className="button button--cta" disabled={busy || !email.trim()} onClick={invite}>
            {busy ? "Sending…" : "Send invite"}
          </button>
        </div>
        <p className="section-subtitle" style={{ marginTop: "0.6rem" }}>They accept or decline the next time they sign in.</p>
        {notice ? <p className="success-text">{notice}</p> : null}
        {error ? <p className="error-text">{error}</p> : null}
      </section>

      {invites.length > 0 ? (
        <section className="panel">
          <h2 className="section-title">Invitations</h2>
          <div className="list-stack" style={{ marginTop: "1rem" }}>
            {invites.map((inv) => (
              <div className="manage-row" key={inv.id}>
                <div>
                  <strong>{inv.email}</strong>
                  <p className="section-subtitle">{inv.role} · <span className="chip" data-tone={statusTone(inv.status)}>{inv.status}</span></p>
                </div>
                {inv.status === "pending" ? (
                  <button type="button" className="ghost-button" onClick={async () => { await managerWithdrawInvitation(inv.id, tenant, token()).catch(() => {}); reload(); }}>
                    Withdraw
                  </button>
                ) : null}
              </div>
            ))}
          </div>
        </section>
      ) : null}

      <section className="panel">
        <h2 className="section-title">Members</h2>
        <div className="list-stack" style={{ marginTop: "1rem" }}>
          {members.map((m) => (
            <MemberRow key={m.user_id} member={m} tenant={tenant} myRole={myRole} ownerCount={ownerCount} onChanged={reload} />
          ))}
        </div>
      </section>
    </>
  );
}

function MemberRow({
  member, tenant, myRole, ownerCount, onChanged,
}: {
  member: WorkspaceMember; tenant: string; myRole: string; ownerCount: number; onChanged: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const token = () => readStoredBearerToken(tenant).trim();
  const canEditOwners = myRole === "owner";
  const roleOptions = canEditOwners ? MEMBER_ROLES : MEMBER_ROLES.filter((r) => r !== "owner");

  const changeRole = async (role: string) => {
    if (member.role === "owner" && ownerCount === 1 && role !== "owner") {
      setError("Keep at least one owner.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await managerUpdateMemberRole(member.user_id, role, tenant, token());
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed");
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (member.role === "owner" && ownerCount === 1) { setError("Keep at least one owner."); return; }
    if (!window.confirm(`Remove ${member.display_name}?`)) return;
    setBusy(true);
    try {
      await managerRemoveMember(member.user_id, tenant, token());
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="manage-row">
      <div>
        <strong>{member.display_name}</strong>
        <p className="section-subtitle">{member.email}</p>
        {error ? <p className="error-text">{error}</p> : null}
      </div>
      <div className="manage-row__actions">
        <select className="manage-input" value={member.role} disabled={busy} onChange={(e) => changeRole(e.target.value)}>
          {roleOptions.map((r) => <option key={r} value={r}>{r}</option>)}
        </select>
        <button type="button" className="ghost-button" disabled={busy} onClick={remove}>Remove</button>
      </div>
    </div>
  );
}

function BoardsTab({ tenant }: { tenant: string }) {
  const [boards, setBoards] = useState<ManageBoard[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [slugInput, setSlugInput] = useState("");
  const [description, setDescription] = useState("");
  const [boardType, setBoardType] = useState(BOARD_TYPES[0]);
  const [isPrivate, setIsPrivate] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const token = () => readStoredBearerToken(tenant).trim();
  const generatedSlug = name.trim().toLowerCase().normalize("NFKD").replace(/[\u0300-\u036f]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  const slug = slugInput.trim() || generatedSlug;

  const reload = useCallback(async () => {
    const t = token();
    if (!t) return;
    try {
      const list = await managerListBoards(tenant, t);
      setBoards(list);
      setSelected((cur) => cur ?? list[0]?.id ?? null);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not load boards.");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tenant]);

  useEffect(() => { void reload(); }, [reload]);

  const board = boards.find((b) => b.id === selected) ?? null;

  async function createBoard(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!name.trim() || !slug) return;
    if (boards.some((existing) => existing.slug === slug)) {
      setError(`A board with the URL slug "${slug}" already exists.`);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const created = await managerCreateBoard({
        tenant_slug: tenant,
        slug,
        name: name.trim(),
        description: description.trim(),
        board_type: boardType,
        is_private: isPrivate,
      }, token());
      setSelected(created.id);
      setCreating(false);
      setName("");
      setSlugInput("");
      setDescription("");
      await reload();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not create board.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page-stack">
      <section className="panel">
        <div className="manage-board-heading">
          <div>
            <h2 className="section-title">Your boards</h2>
            <p className="section-subtitle">Public boards appear at <Link className="text-link" href={`/${encodeURIComponent(tenant)}`}>/{tenant}</Link>. Private boards are for workspace members.</p>
          </div>
          <button className="button button--cta" type="button" onClick={() => setCreating((value) => !value)}>{creating ? "Cancel" : "Create board"}</button>
        </div>
        {creating ? <form className="manage-fields" style={{ marginTop: "1rem" }} onSubmit={(event) => void createBoard(event)}>
          <label className="manage-label">Board name<input className="manage-input" value={name} onChange={(event) => setName(event.target.value)} placeholder="Product ideas" required /></label>
          <label className="manage-label">URL slug<input className="manage-input" value={slugInput} onChange={(event) => setSlugInput(event.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ""))} placeholder={generatedSlug || "product-ideas"} pattern="[a-z0-9]+(?:-[a-z0-9]+)*" required={!generatedSlug} /></label>
          <label className="manage-label">Description<textarea className="manage-input" rows={2} value={description} onChange={(event) => setDescription(event.target.value)} /></label>
          <label className="manage-label">Type<select className="manage-input" value={boardType} onChange={(event) => setBoardType(event.target.value)}>{BOARD_TYPES.map((type) => <option key={type} value={type}>{type}</option>)}</select></label>
          <label className="manage-check"><input type="checkbox" checked={isPrivate} onChange={(event) => setIsPrivate(event.target.checked)} /> Private board (members only)</label>
          <button className="button button--cta" type="submit" disabled={busy || !name.trim() || !slug}>{busy ? "Creating…" : "Create board"}</button>
        </form> : null}
        {error ? <p className="error-text" role="alert">{error}</p> : null}
      </section>
      <section className="dashboard-grid">
      <section className="panel">
        <h2 className="section-title">Boards</h2>
        <div className="list-stack" style={{ marginTop: "1rem" }}>
          {boards.map((b) => (
            <button type="button" key={b.id} className={`manage-board-tab${b.id === selected ? " manage-board-tab--active" : ""}`} onClick={() => setSelected(b.id)}>
              <strong>{b.name}</strong>
              <span className="section-subtitle">{b.board_type}{b.is_private ? " · private" : ""}</span>
            </button>
          ))}
        </div>
      </section>
      {board ? <BoardEditor key={board.id} board={board} tenant={tenant} onSaved={reload} /> : (
        <section className="panel"><p className="section-subtitle">Select a board to edit.</p></section>
      )}
      </section>
    </div>
  );
}

function BoardEditor({ board, tenant, onSaved }: { board: ManageBoard; tenant: string; onSaved: () => void }) {
  const [name, setName] = useState(board.name);
  const [description, setDescription] = useState(board.description ?? "");
  const [boardType, setBoardType] = useState(board.board_type);
  const [isPrivate, setIsPrivate] = useState(board.is_private);
  const [bg, setBg] = useState(board.background_color ?? "#fff1ea");
  const [sections, setSections] = useState<DashboardSection[]>(board.dashboard_sections ?? ["progress", "latest", "top"]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const toggle = (s: DashboardSection) =>
    setSections((cur) => (cur.includes(s) ? cur.filter((x) => x !== s) : SECTIONS.map((x) => x.id).filter((x) => x === s || cur.includes(x))));

  const save = async () => {
    setBusy(true); setError(null); setNotice(null);
    try {
      await managerUpdateBoard(board.id, {
        name: name.trim(),
        description: description.trim() || undefined,
        board_type: boardType,
        is_private: isPrivate,
        background_color: bg || undefined,
        dashboard_sections: sections,
        icon_url: board.icon_url,
      }, readStoredBearerToken(tenant).trim());
      setNotice("Saved.");
      onSaved();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to save");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="panel">
      <h2 className="section-title">{board.name}</h2>
      <div className="manage-board-links">
        <Link className="button" href={`/${encodeURIComponent(tenant)}/boards/${encodeURIComponent(board.slug)}`}>Open board</Link>
        {!board.is_private ? <button className="button" type="button" onClick={() => {
          const url = `${window.location.origin}/${encodeURIComponent(tenant)}/boards/${encodeURIComponent(board.slug)}`;
          void navigator.clipboard.writeText(url).then(() => setNotice("Public board link copied.")).catch(() => setError("Could not copy link. Open the board and copy its URL."));
        }}>Copy public link</button> : <span className="section-subtitle">Private: workspace members only</span>}
      </div>
      <div className="manage-fields" style={{ marginTop: "1rem" }}>
        <label className="manage-label">Name<input className="manage-input" value={name} onChange={(e) => setName(e.target.value)} /></label>
        <label className="manage-label">Description<textarea className="manage-input" rows={3} value={description} onChange={(e) => setDescription(e.target.value)} /></label>
        <label className="manage-label">Type
          <select className="manage-input" value={boardType} onChange={(e) => setBoardType(e.target.value)}>
            {BOARD_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
        </label>
        <label className="manage-label">Background color
          <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
            <input type="color" value={/^#[0-9a-fA-F]{6}$/.test(bg) ? bg : "#fff1ea"} onChange={(e) => setBg(e.target.value)} />
            <input className="manage-input" value={bg} onChange={(e) => setBg(e.target.value)} placeholder="#fff1ea" />
          </div>
        </label>
        <label className="manage-check"><input type="checkbox" checked={isPrivate} onChange={(e) => setIsPrivate(e.target.checked)} /> Private board (members only)</label>
        <div className="manage-label">
          Dashboard sections
          <div className="manage-sections">
            {SECTIONS.map((s) => (
              <label key={s.id} className="manage-check">
                <input type="checkbox" checked={sections.includes(s.id)} onChange={() => toggle(s.id)} /> {s.label}
              </label>
            ))}
          </div>
        </div>
      </div>
      <div style={{ marginTop: "1rem", display: "flex", gap: "0.6rem", alignItems: "center" }}>
        <button type="button" className="button button--cta" disabled={busy || !name.trim()} onClick={save}>{busy ? "Saving…" : "Save changes"}</button>
        {notice ? <span className="success-text">{notice}</span> : null}
      </div>
      {error ? <p className="error-text">{error}</p> : null}
    </section>
  );
}

function SsoTab({ tenant }: { tenant: string }) {
  const [cfg, setCfg] = useState<SsoConfig | null>(null);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const token = () => readStoredBearerToken(tenant).trim();

  useEffect(() => {
    const t = token();
    if (!t) return;
    getSsoConfig(tenant, t).then(setCfg).catch(() => setCfg(null));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tenant]);

  const endpoint = `${API_BASE_URL}/api/auth/sso-session`;
  const secret = cfg?.secret ?? "";

  const enableOrRotate = async () => {
    setBusy(true);
    try {
      setCfg(await regenerateSsoSecret(tenant, token()));
    } finally {
      setBusy(false);
    }
  };
  const turnOff = async () => {
    if (!window.confirm("Disable SSO? Existing sessions keep working; new SSO logins are rejected.")) return;
    setBusy(true);
    try {
      setCfg(await disableSso(tenant, token()));
    } finally {
      setBusy(false);
    }
  };
  const copy = (text: string) => {
    navigator.clipboard?.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const snippet = `// On YOUR server (Node) — never expose the secret to the browser.
import jwt from "jsonwebtoken";

const token = jwt.sign(
  { sub: user.id, email: user.email, name: user.name },
  process.env.HOWLLO_SSO_SECRET,          // the workspace secret above
  { algorithm: "HS256", expiresIn: "1h" },
);

const res = await fetch("${endpoint}", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ tenant_slug: "${tenant}", token }),
});
const { session_token } = await res.json();
// Use session_token as the Bearer token for the board — no login screen.`;

  return (
    <section className="panel">
      <h2 className="section-title">End-user SSO</h2>
      <p className="section-subtitle" style={{ marginTop: "0.3rem" }}>
        Let your already-signed-in users post &amp; vote without a howllo login. Your backend signs a
        token with the workspace secret; howllo trusts it and mints a session.
      </p>

      {!cfg?.enabled ? (
        <div style={{ marginTop: "1rem" }}>
          <button type="button" className="button button--cta" disabled={busy} onClick={enableOrRotate}>
            {busy ? "Enabling…" : "Enable SSO"}
          </button>
        </div>
      ) : (
        <div className="manage-fields" style={{ marginTop: "1rem" }}>
          <label className="manage-label">
            Workspace secret (keep on your server only)
            <div className="sso-secret">
              <code>{secret}</code>
              <button type="button" className="text-button" onClick={() => copy(secret)}>
                {copied ? "Copied" : "Copy"}
              </button>
            </div>
          </label>
          <label className="manage-label">
            Endpoint
            <div className="sso-secret">
              <code>{endpoint}</code>
            </div>
          </label>
          <div className="manage-label">
            Integration (Node)
            <pre className="sso-code">
              <code>{snippet}</code>
            </pre>
          </div>
          <div style={{ display: "flex", gap: "0.6rem" }}>
            <button type="button" className="ghost-button" disabled={busy} onClick={enableOrRotate}>
              Regenerate secret
            </button>
            <button type="button" className="ghost-button button--danger" disabled={busy} onClick={turnOff}>
              Disable SSO
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
