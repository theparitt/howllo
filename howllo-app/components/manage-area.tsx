"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { RooiamInlineLogin } from "@/components/rooiam-inline-login";
import { ENABLED_AUTH_PROVIDERS } from "@/lib/auth-provider";
import {
  disableSso,
  getMyWorkspaceRole,
  getBoardCategories,
  getTags,
  getTenantManagementSettings,
  getSsoConfig,
  regenerateSsoSecret,
  type SsoConfig,
  managerCreateInvitation,
  managerCreateBoard,
  managerDeleteBoard,
  managerGetBoardSummary,
  managerListBoards,
  managerGetBoardCount,
  managerListInvitations,
  managerListMembers,
  managerRemoveMember,
  managerUpdateBoard,
  managerUpdateMemberRole,
  managerWithdrawInvitation,
  type BoardSummary,
  createPost,
} from "@/lib/api";
import { BOARD_PRESETS, boardKind, boardPreset } from "@/lib/board-experience";
import {
  getCurrentWorkspaceSlug,
  readStoredBearerToken,
  subscribeToBearerTokenChange,
} from "@/components/dev-auth-panel";
import type { BoardCategory, ManageBoard, MyInvitation, Tag, WorkspaceMember } from "@/lib/types";
import { API_BASE_URL } from "@/lib/config";
import { uploadImage } from "@/lib/api";
import { ParticipantsTab } from "./participants-tab";
import { ModerationTab } from "./moderation-tab";
import { BrandingTab } from "./branding-tab";
import { SecurityTab } from "./security-tab";
import { CustomerSignInTab } from "./customer-signin-tab";
import { WorkspacePluginsTab } from "./workspace-plugins-tab";
import { prepareBrandImage } from "../lib/prepare-brand-image";
import { BoardTaxonomyEditor } from "./board-taxonomy-editor";
import { BoardExtras } from "./board-extras";
import { BoardTopics } from "./board-topics";
import { LIST_PAGE_SIZE, ListPager, ListSearch } from "./list-controls";

const INVITE_ROLES = ["admin", "moderator"];
const MEMBER_ROLES = ["owner", "admin", "moderator", "member"];
const PUBLIC_WEB_URL = (process.env.NEXT_PUBLIC_HOWLLO_PUBLIC_WEB_URL || "http://localhost:7703").replace(/\/$/, "");
type ManageTab = "team" | "boards" | "branding" | "plugins" | "security" | "signin" | "external_sso" | "participants" | "moderation";
const NAV_ITEMS: { key: ManageTab; label: string; group: string; icon: string }[] = [
  { key: "boards", label: "Boards", group: "Workspace", icon: "M4 5h16v14H4zM4 10h16M10 10v9" },
  { key: "branding", label: "Board site", group: "Workspace", icon: "M3 5h18v14H3zM3 9h18M7 14h4" },
  { key: "plugins", label: "Plugins", group: "Workspace", icon: "M9 3v5H4v5h5v5h5v-5h5V8h-5V3z" },
  { key: "team", label: "Staff", group: "People", icon: "M16 19v-1a4 4 0 00-8 0v1M12 11a3 3 0 100-6 3 3 0 000 6" },
  { key: "participants", label: "Board members", group: "People", icon: "M4 19v-1a4 4 0 018 0v1M8 11a3 3 0 100-6 3 3 0 000 6M16 8h5M16 12h5" },
  { key: "moderation", label: "Moderation", group: "People", icon: "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6zM9 12l2 2 4-4" },
  { key: "security", label: "Security", group: "Settings", icon: "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z" },
  { key: "signin", label: "Customer sign-in", group: "Settings", icon: "M10 17l5-5-5-5M15 12H3M13 4h5a2 2 0 012 2v12a2 2 0 01-2 2h-5" },
  { key: "external_sso", label: "External SSO", group: "Settings", icon: "M4 8h16M4 16h16M8 4v16" },
];
const BOARD_NAV_ITEMS = new Set<ManageTab>(["boards", "branding", "participants", "moderation"]);

function NavIcon({ path }: { path: string }) {
  return <svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={path} /></svg>;
}

function publicBoardUrl(tenant: string, boardSlug: string): string {
  return `${PUBLIC_WEB_URL}/${encodeURIComponent(tenant)}/boards/${encodeURIComponent(boardSlug)}`;
}

function BoardTypePicker({ value, onChange, group }: { value: string; onChange: (type: string) => void; group: string }) {
  return <div className="board-kind-picker" role="radiogroup" aria-label="Board type">
    {BOARD_PRESETS.map((preset) => <label key={preset.value} className={`board-kind-picker__option${value === preset.value ? " board-kind-picker__option--active" : ""}`}>
      <input type="radio" name={group} value={preset.value} checked={value === preset.value} onChange={() => onChange(preset.value)} />
      <strong>{preset.label}</strong><span>{preset.description}</span>
    </label>)}
  </div>;
}

function statusTone(status: string): string {
  if (status === "accepted") return "green";
  if (status === "pending") return "blue";
  return "warm";
}

export function ManageArea() {
  const [tenant, setTenant] = useState<string | null>(null);
  const [role, setRole] = useState<string | null | undefined>(undefined);
  const [tab, setTab] = useState<ManageTab>("boards");
  const [collapsed, setCollapsed] = useState(false);
  const [boardList, setBoardList] = useState<ManageBoard[] | null>(null);
  const [moderatorBoardCount, setModeratorBoardCount] = useState<number | null>(null);
  const [boardLoadError, setBoardLoadError] = useState("");
  const [boardRefresh, setBoardRefresh] = useState(0);

  useEffect(() => { setCollapsed(window.localStorage.getItem("howllo.staff.sidebar.collapsed") === "true"); }, []);
  const toggleSidebar = () => setCollapsed((value) => { window.localStorage.setItem("howllo.staff.sidebar.collapsed", String(!value)); return !value; });

  useEffect(() => {
    const resolve = () => setTenant(getCurrentWorkspaceSlug());
    resolve();
    return subscribeToBearerTokenChange(resolve);
  }, []);

  useEffect(() => {
    if (!tenant) return;
    let cancelled = false;
    setRole(undefined);
    setBoardList(null);
    setModeratorBoardCount(null);
    const token = readStoredBearerToken(tenant).trim();
    if (!token) {
      setRole(null);
      return;
    }
    getMyWorkspaceRole(tenant, token)
      .then((value) => { if (!cancelled) setRole(value); })
      .catch(() => { if (!cancelled) setRole(null); });
    return () => { cancelled = true; };
  }, [tenant]);

  useEffect(() => {
    if (!tenant || role !== "moderator") return;
    let cancelled = false;
    setModeratorBoardCount(null);
    setBoardLoadError("");
    managerGetBoardCount(tenant, readStoredBearerToken(tenant).trim())
      .then((count) => { if (!cancelled) setModeratorBoardCount(count); })
      .catch((cause) => { if (!cancelled) setBoardLoadError(cause instanceof Error ? cause.message : "Could not load boards."); });
    return () => { cancelled = true; };
  }, [tenant, role, boardRefresh]);

  useEffect(() => {
    if (!tenant || (role !== "owner" && role !== "admin")) return;
    let cancelled = false;
    setBoardList(null);
    setBoardLoadError("");
    managerListBoards(tenant, readStoredBearerToken(tenant).trim())
      .then((boards) => { if (!cancelled) setBoardList(boards); })
      .catch((cause) => { if (!cancelled) setBoardLoadError(cause instanceof Error ? cause.message : "Could not load boards."); });
    return () => { cancelled = true; };
  }, [tenant, role, boardRefresh]);

  useEffect(() => {
    if (role === "moderator") setTab("moderation");
  }, [role]);

  useEffect(() => {
    if (boardList?.length === 0 && BOARD_NAV_ITEMS.has(tab) && tab !== "boards") setTab("boards");
  }, [boardList, tab]);

  if (role === undefined) {
    return <section className="panel"><p className="section-subtitle">Loading…</p></section>;
  }

  const canManage = role === "owner" || role === "admin" || role === "moderator";
  if (!canManage) {
    if (tenant && !readStoredBearerToken(tenant).trim() && ENABLED_AUTH_PROVIDERS.length === 1 && ENABLED_AUTH_PROVIDERS[0] === "rooiam") {
      return <section className="rooiam-inline-login" aria-label="Workspace staff sign in">
        <h1 className="rooiam-inline-login__title">Workspace staff sign in</h1>
        <p className="section-subtitle">Sign in with RooIAM.</p>
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

  if (role !== "moderator" && boardList === null) {
    return <section className="panel">{boardLoadError ? <p className="error-text" role="alert">{boardLoadError} <button type="button" className="ghost-button" onClick={() => setBoardRefresh((value) => value + 1)}>Retry</button></p> : <p className="section-subtitle">Loading workspace…</p>}</section>;
  }

  if (role === "moderator" && moderatorBoardCount === null) {
    return <section className="panel">{boardLoadError ? <p className="error-text" role="alert">{boardLoadError} <button type="button" className="ghost-button" onClick={() => setBoardRefresh((value) => value + 1)}>Retry</button></p> : <p className="section-subtitle">Loading workspace…</p>}</section>;
  }
  if (role === "moderator" && moderatorBoardCount === 0) {
    return <section className="panel empty-state"><h1 className="empty-state__title">No boards yet</h1><p className="empty-state__copy">An owner or admin can create the first board.</p></section>;
  }

  const noBoards = boardList?.length === 0;
  const visibleNav = NAV_ITEMS.filter((item) => (role !== "moderator" || item.key === "moderation") && (!noBoards || !BOARD_NAV_ITEMS.has(item.key)));

  return (
    <div className={`manage-layout${collapsed ? " manage-layout--collapsed" : ""}`}>
      <aside className="manage-sidebar" aria-label="Workspace navigation">
        <div className="manage-sidebar__top"><span className="manage-sidebar__title">Workspace</span><button type="button" className="manage-sidebar__toggle" onClick={toggleSidebar} aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"} title={collapsed ? "Expand sidebar" : "Collapse sidebar"}><NavIcon path={collapsed ? "M9 5l7 7-7 7" : "M15 5l-7 7 7 7"} /></button></div>
        {noBoards ? <button type="button" className={`manage-sidebar__create${tab === "boards" ? " manage-sidebar__create--active" : ""}`} aria-label="Create a board" title={collapsed ? "Create a board" : undefined} onClick={() => setTab("boards")}><NavIcon path="M12 5v14M5 12h14" /><span>Create a board</span></button> : null}
        {Array.from(new Set(visibleNav.map((item) => item.group))).map((group) => <div className="manage-sidebar__group" key={group}>
          <span className="manage-sidebar__group-label">{group}</span>
          {visibleNav.filter((item) => item.group === group).map((item) => <button key={item.key} type="button" className={`manage-sidebar__item${tab === item.key ? " manage-sidebar__item--active" : ""}`} aria-current={tab === item.key ? "page" : undefined} aria-label={item.label} title={collapsed ? item.label : undefined} onClick={() => setTab(item.key)}><NavIcon path={item.icon} /><span>{item.label}</span></button>)}
        </div>)}
      </aside>
      <div className="manage-layout__content">
      <header className="manage-layout__heading"><h1 className="page-title">{noBoards && tab === "boards" ? "Create your first board" : NAV_ITEMS.find((item) => item.key === (role === "moderator" ? "moderation" : tab))?.label}</h1>{tab === "branding" ? <p>Logo, colors, and pages your customers see.</p> : null}</header>
      {tenant ? (
        tab === "moderation" || role === "moderator" ? (
          <ModerationTab tenant={tenant} />
        ) : tab === "participants" ? (
          <ParticipantsTab tenant={tenant} />
        ) : tab === "branding" ? (
          <BrandingTab tenant={tenant} />
        ) : tab === "plugins" ? (
          <WorkspacePluginsTab tenant={tenant} />
        ) : tab === "security" ? (
          <SecurityTab tenant={tenant} />
        ) : tab === "team" ? (
          <TeamTab tenant={tenant} myRole={role!} />
        ) : tab === "boards" ? (
          <BoardsTab tenant={tenant} initialBoards={boardList ?? []} onBoardsChange={setBoardList} />
        ) : tab === "signin" ? (
          <CustomerSignInTab tenant={tenant} />
        ) : (
          <SsoTab tenant={tenant} />
        )
      ) : null}
      </div>
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
  const [memberQuery, setMemberQuery] = useState("");
  const [memberRole, setMemberRole] = useState("all");
  const [memberPage, setMemberPage] = useState(1);
  const [inviteQuery, setInviteQuery] = useState("");
  const [invitePage, setInvitePage] = useState(1);

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
      setNotice(`Invitation created for ${email.trim()}. Ask them to sign in to Howllo App with RooIAM.`);
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
  const visibleMembers = useMemo(() => members.filter((member) => {
    const query = memberQuery.trim().toLowerCase();
    return (!query || `${member.display_name} ${member.email}`.toLowerCase().includes(query)) && (memberRole === "all" || member.role === memberRole);
  }), [members, memberQuery, memberRole]);
  const visibleInvites = useMemo(() => invites.filter((invite) => `${invite.email} ${invite.role} ${invite.status}`.toLowerCase().includes(inviteQuery.trim().toLowerCase())), [invites, inviteQuery]);
  const safeMemberPage = Math.min(memberPage, Math.max(1, Math.ceil(visibleMembers.length / LIST_PAGE_SIZE)));
  const safeInvitePage = Math.min(invitePage, Math.max(1, Math.ceil(visibleInvites.length / LIST_PAGE_SIZE)));

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
        <p className="section-subtitle" style={{ marginTop: "0.6rem" }}>They accept or decline in Howllo App after signing in with RooIAM. Email delivery is not configured.</p>
        {notice ? <p className="success-text">{notice}</p> : null}
        {error ? <p className="error-text">{error}</p> : null}
      </section>

      {invites.length > 0 ? (
        <section className="panel">
          <h2 className="section-title">Invitations</h2>
          <ListSearch value={inviteQuery} onChange={(value) => { setInviteQuery(value); setInvitePage(1); }} placeholder="Search invitations" />
          <div className="list-stack" style={{ marginTop: "1rem" }}>
            {visibleInvites.slice((safeInvitePage - 1) * LIST_PAGE_SIZE, safeInvitePage * LIST_PAGE_SIZE).map((inv) => (
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
            {visibleInvites.length === 0 ? <p className="section-subtitle">No matching invitations.</p> : null}
          </div>
          <ListPager page={safeInvitePage} total={visibleInvites.length} onPage={setInvitePage} />
        </section>
      ) : null}

      <section className="panel">
        <h2 className="section-title">Members</h2>
        <ListSearch value={memberQuery} onChange={(value) => { setMemberQuery(value); setMemberPage(1); }} placeholder="Search staff by name or email"><select className="manage-input" aria-label="Filter staff by role" value={memberRole} onChange={(event) => { setMemberRole(event.target.value); setMemberPage(1); }}><option value="all">All roles</option>{MEMBER_ROLES.map((role) => <option key={role} value={role}>{role}</option>)}</select></ListSearch>
        <div className="list-stack" style={{ marginTop: "1rem" }}>
          {visibleMembers.slice((safeMemberPage - 1) * LIST_PAGE_SIZE, safeMemberPage * LIST_PAGE_SIZE).map((m) => (
            <MemberRow key={m.user_id} member={m} tenant={tenant} myRole={myRole} ownerCount={ownerCount} onChanged={reload} />
          ))}
          {visibleMembers.length === 0 ? <p className="section-subtitle">No matching staff.</p> : null}
        </div>
        <ListPager page={safeMemberPage} total={visibleMembers.length} onPage={setMemberPage} />
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

function BoardsTab({ tenant, initialBoards, onBoardsChange }: { tenant: string; initialBoards: ManageBoard[]; onBoardsChange: (boards: ManageBoard[]) => void }) {
  const [boards, setBoards] = useState<ManageBoard[]>(initialBoards);
  const [loading, setLoading] = useState(true);
  const [workspacePublished, setWorkspacePublished] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [slugInput, setSlugInput] = useState("");
  const [description, setDescription] = useState("");
  const [boardType, setBoardType] = useState<string>(BOARD_PRESETS[0].value);
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
      const [list, settings] = await Promise.all([managerListBoards(tenant, t), getTenantManagementSettings(tenant, t)]);
      setBoards(list);
      onBoardsChange(list);
      setWorkspacePublished(settings.is_published);
      setSelected((cur) => (cur && list.some((item) => item.id === cur) ? cur : list[0]?.id ?? null));
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not load boards.");
    } finally { setLoading(false); }
  }, [tenant, onBoardsChange]);

  useEffect(() => { void reload(); }, [reload]);

  const board = boards.find((b) => b.id === selected) ?? null;

  const afterDelete = async () => {
    setSelected(null);
    await reload();
  };

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
        allow_votes: boardPreset(boardType).defaultVotes,
        allow_comments: boardPreset(boardType).defaultComments,
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

  if (loading) return <section className="panel"><p className="section-subtitle">Loading boards…</p></section>;

  return (
    <div className="page-stack">
      <section className="panel">
        <div className="manage-board-heading">
          <div>
            <h2 className="section-title">{boards.length === 0 ? "Start with a board" : "Your boards"}</h2>
            <p className="section-subtitle">{boards.length === 0 ? "Give your board a name and choose how people will use it." : "Create boards one at a time. Each starts as a draft until you publish it."}</p>
          </div>
          {boards.length > 0 ? <button className="button button--cta" type="button" onClick={() => setCreating((value) => !value)}>{creating ? "Cancel" : "Create board"}</button> : null}
        </div>
        {creating || boards.length === 0 ? <form className="manage-fields" style={{ marginTop: "1rem" }} onSubmit={(event) => void createBoard(event)}>
          <label className="manage-label">Board name<input className="manage-input" value={name} onChange={(event) => setName(event.target.value)} placeholder="Product ideas" required /></label>
          <label className="manage-label">URL slug<input className="manage-input" value={slugInput} onChange={(event) => setSlugInput(event.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ""))} placeholder={generatedSlug || "product-ideas"} pattern="[a-z0-9]+(?:-[a-z0-9]+)*" required={!generatedSlug} /></label>
          <label className="manage-label">Description<textarea className="manage-input" rows={2} value={description} onChange={(event) => setDescription(event.target.value)} /></label>
          <div className="manage-label">Board type<BoardTypePicker value={boardType} onChange={setBoardType} group="create-board-kind" /></div>
          <label className="manage-check"><input type="checkbox" checked={isPrivate} onChange={(event) => setIsPrivate(event.target.checked)} /> Private board (members only)</label>
          <button className="button button--cta" type="submit" disabled={busy || !name.trim() || !slug}>{busy ? "Creating…" : "Create board"}</button>
        </form> : null}
        {error ? <p className="error-text" role="alert">{error}</p> : null}
      </section>
      {boards.length > 0 ? <section className="dashboard-grid">
        <section className="panel">
          <h2 className="section-title">Boards</h2>
          <div className="list-stack" style={{ marginTop: "1rem" }}>
            {boards.map((b) => (
              <button type="button" key={b.id} className={`manage-board-tab${b.id === selected ? " manage-board-tab--active" : ""}`} onClick={() => setSelected(b.id)}>
                <strong>{b.name}</strong>
                <span className="section-subtitle">{b.board_type}{b.is_private ? " · private" : ""} · {b.is_enabled ? workspacePublished ? "Active" : "Ready" : b.first_enabled_at ? "Paused" : "Draft"}</span>
              </button>
            ))}
          </div>
        </section>
        {board ? <BoardEditor key={board.id} board={board} tenant={tenant} workspacePublished={workspacePublished} onSaved={reload} onDeleted={afterDelete} /> : (
          <section className="panel"><p className="section-subtitle">Select a board to edit.</p></section>
        )}
      </section> : null}
    </div>
  );
}

function BoardEditor({ board, tenant, workspacePublished, onSaved, onDeleted }: { board: ManageBoard; tenant: string; workspacePublished: boolean; onSaved: () => Promise<void>; onDeleted: () => Promise<void> }) {
  const [name, setName] = useState(board.name);
  const [description, setDescription] = useState(board.description ?? "");
  const [boardType, setBoardType] = useState<string>(boardKind(board.board_type));
  const [introText, setIntroText] = useState(board.intro_text ?? "");
  const [allowVotes, setAllowVotes] = useState(board.allow_votes);
  const [allowComments, setAllowComments] = useState(board.allow_comments);
  const [isPrivate, setIsPrivate] = useState(board.is_private);
  const [bg, setBg] = useState(board.background_color ?? "#fff1ea");
  const [iconUrl, setIconUrl] = useState(board.icon_url);
  const [headerImageUrl, setHeaderImageUrl] = useState(board.header_image_url);
  const [backgroundImageUrl, setBackgroundImageUrl] = useState(board.background_image_url);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteSummary, setDeleteSummary] = useState<BoardSummary | null>(null);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [confirmSlug, setConfirmSlug] = useState("");

  const save = async () => {
    setBusy(true); setError(null); setNotice(null);
    try {
      await managerUpdateBoard(board.id, {
        name: name.trim(),
        description: description.trim() || undefined,
        board_type: boardType,
        intro_text: introText,
        allow_votes: allowVotes,
        allow_comments: allowComments,
        is_private: isPrivate,
        is_enabled: board.is_enabled,
        background_color: bg || undefined,
        dashboard_sections: board.dashboard_sections,
        icon_url: iconUrl,
        header_image_url: headerImageUrl,
        background_image_url: backgroundImageUrl,
      }, readStoredBearerToken(tenant).trim());
      setNotice("Saved.");
      await onSaved();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to save");
    } finally {
      setBusy(false);
    }
  };

  const setPublication = async (enabled: boolean) => {
    setBusy(true); setError(null); setNotice(null);
    try {
      await managerUpdateBoard(board.id, {
        name: board.name,
        description: board.description,
        board_type: board.board_type,
        is_private: board.is_private,
        is_enabled: enabled,
        background_color: board.background_color,
        dashboard_sections: board.dashboard_sections,
        icon_url: board.icon_url,
        header_image_url: board.header_image_url,
        background_image_url: board.background_image_url,
      }, readStoredBearerToken(tenant).trim());
      await onSaved();
      setNotice(enabled ? workspacePublished ? "Board is active." : "Board is ready. Publish the workspace to make it visible." : "Board is paused. Its posts are hidden until you resume it.");
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not update board."); }
    finally { setBusy(false); }
  };

  const openDelete = async () => {
    setDeleteOpen(true); setDeleteSummary(null); setDeleteLoading(true); setConfirmSlug(""); setError(null);
    try {
      setDeleteSummary(await managerGetBoardSummary(board.id, tenant, readStoredBearerToken(tenant).trim()));
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not check board contents."); }
    finally { setDeleteLoading(false); }
  };

  const deleteBoard = async () => {
    if (confirmSlug !== board.slug || !deleteSummary) return;
    setBusy(true); setError(null);
    try {
      await managerDeleteBoard(board.id, readStoredBearerToken(tenant).trim());
      await onDeleted();
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not delete board."); }
    finally { setBusy(false); }
  };

  return (
    <section className="panel">
      <div className="manage-board-heading"><h2 className="section-title">{board.name}</h2><strong>{board.is_enabled ? workspacePublished ? "Active" : "Ready" : board.first_enabled_at ? "Paused" : "Draft"}</strong></div>
      <div className="manage-board-links">
        {board.is_enabled && workspacePublished ? <a className="button" href={publicBoardUrl(tenant, board.slug)}>Open board ↗</a> : null}
        {!board.is_private && board.is_enabled && workspacePublished ? <button className="button" type="button" onClick={() => {
          const url = publicBoardUrl(tenant, board.slug);
          void navigator.clipboard.writeText(url).then(() => setNotice("Public board link copied.")).catch(() => setError("Could not copy link. Open the board and copy its URL."));
        }}>Copy board link</button> : <span className="section-subtitle">{!board.is_enabled ? "Hidden from visitors" : !workspacePublished ? "Publish this workspace in Board site to share its boards." : "Private: workspace members only"}</span>}
      </div>
      <div className="manage-fields" style={{ marginTop: "1rem" }}>
        <label className="manage-label">Name<input className="manage-input" value={name} onChange={(e) => setName(e.target.value)} /></label>
        <label className="manage-label">Description<textarea className="manage-input" rows={3} value={description} onChange={(e) => setDescription(e.target.value)} /></label>
        <div className="manage-label">Board type<BoardTypePicker value={boardType} group="edit-board-kind" onChange={(next) => { setBoardType(next); setAllowVotes(boardPreset(next).defaultVotes); setAllowComments(boardPreset(next).defaultComments); }} /></div>
        <label className="manage-label">Board introduction<textarea className="manage-input" rows={2} maxLength={240} value={introText} onChange={(e) => setIntroText(e.target.value)} placeholder="Optional guidance shown above posts" /></label>
        <label className="manage-check"><input type="checkbox" checked={allowVotes} onChange={(e) => setAllowVotes(e.target.checked)} /> {boardKind(boardType) === "bug-reports" ? "Let visitors mark a bug as affecting them" : boardKind(boardType) === "discussions" ? "Let visitors like discussions" : boardKind(boardType) === "announcements" ? "Let visitors mark updates helpful" : "Let visitors vote on ideas"}</label>
        <label className="manage-check"><input type="checkbox" checked={allowComments} onChange={(e) => setAllowComments(e.target.checked)} /> {boardKind(boardType) === "discussions" ? "Let visitors reply" : boardKind(boardType) === "announcements" ? "Let visitors respond" : "Let visitors comment"}</label>
        <label className="manage-label">Background color
          <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
            <input type="color" value={/^#[0-9a-fA-F]{6}$/.test(bg) ? bg : "#fff1ea"} onChange={(e) => setBg(e.target.value)} />
            <input className="manage-input" value={bg} onChange={(e) => setBg(e.target.value)} placeholder="#fff1ea" />
          </div>
        </label>
        <div className="manage-label">Header image
          {headerImageUrl ? <img src={headerImageUrl} alt="Board header preview" style={{ display: "block", width: "100%", maxHeight: 180, objectFit: "cover", borderRadius: 12, margin: "0.6rem 0" }} /> : <span className="section-subtitle">Shown above this board’s posts.</span>}
          <input className="manage-input" type="file" accept="image/png,image/jpeg,image/webp" disabled={busy} onChange={async (event) => {
            const file = event.target.files?.[0]; event.target.value = ""; if (!file) return;
            setBusy(true); setError(null);
            try { setHeaderImageUrl(await uploadImage(await prepareBrandImage(file, 1800, 600, "image/webp"), readStoredBearerToken(tenant).trim(), tenant)); setNotice("Header uploaded. Save changes to publish it."); }
            catch (cause) { setError(cause instanceof Error ? cause.message : "Could not upload header."); }
            finally { setBusy(false); }
          }} />
          <span className="section-subtitle">Use a wide image. PNG, JPG or WebP · up to 5 MB · fitted within 1800 × 600.</span>
          {headerImageUrl ? <button type="button" className="ghost-button" disabled={busy} onClick={() => setHeaderImageUrl(null)}>Remove header</button> : null}
        </div>
        <div className="manage-label">Background image
          {backgroundImageUrl ? <img src={backgroundImageUrl} alt="Board background preview" style={{ display: "block", width: "100%", maxHeight: 130, objectFit: "cover", borderRadius: 12, margin: "0.6rem 0" }} /> : <span className="section-subtitle">A subtle image behind the board content. Leave empty for the workspace color.</span>}
          <input className="manage-input" type="file" accept="image/png,image/jpeg,image/webp" disabled={busy} onChange={async (event) => {
            const file = event.target.files?.[0]; event.target.value = ""; if (!file) return;
            setBusy(true); setError(null);
            try { setBackgroundImageUrl(await uploadImage(await prepareBrandImage(file, 2000, 1400, "image/webp"), readStoredBearerToken(tenant).trim(), tenant)); setNotice("Background uploaded. Save changes to publish it."); }
            catch (cause) { setError(cause instanceof Error ? cause.message : "Could not upload background."); }
            finally { setBusy(false); }
          }} />
          <span className="section-subtitle">PNG, JPG or WebP · up to 5 MB · fitted within 2000 × 1400.</span>
          {backgroundImageUrl ? <button type="button" className="ghost-button" disabled={busy} onClick={() => setBackgroundImageUrl(null)}>Remove background</button> : null}
        </div>
        <div className="manage-label">Board icon
          {iconUrl ? <img src={iconUrl} alt="Current board icon" className="branding-board-icon" /> : <span className="section-subtitle">No custom icon</span>}
          <input className="manage-input" type="file" accept="image/png,image/jpeg,image/webp" disabled={busy} onChange={async (event) => {
            const file = event.target.files?.[0]; event.target.value = "";
            if (!file) return;
            setBusy(true); setError(null);
            try {
              const prepared = await prepareBrandImage(file, 512, 512);
              setIconUrl(await uploadImage(prepared, readStoredBearerToken(tenant).trim(), tenant));
              setNotice("Icon uploaded. Save changes to publish it.");
            } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not upload icon."); }
            finally { setBusy(false); }
          }} />
          <span className="section-subtitle">PNG, JPG or WebP · up to 5 MB. Resized to fit 512 × 512.</span>
          {iconUrl ? <button type="button" className="ghost-button" disabled={busy} onClick={() => setIconUrl(null)}>Remove icon</button> : null}
        </div>
        <label className="manage-check"><input type="checkbox" checked={isPrivate} onChange={(e) => setIsPrivate(e.target.checked)} /> Private board (members only)</label>
      </div>
      <BoardTaxonomyEditor boardId={board.id} tenant={tenant} />
      <BoardExtras boardId={board.id} tenant={tenant} />
      {board.is_enabled && workspacePublished ? <BoardTopics boardSlug={board.slug} tenant={tenant} /> : null}
      <div className="manage-board-links">
        <button type="button" className="button button--cta" disabled={busy || !name.trim()} onClick={save}>{busy ? "Saving…" : "Save changes"}</button>
        <button type="button" className="ghost-button" disabled={busy} onClick={() => void setPublication(!board.is_enabled)}>{board.is_enabled ? "Pause board" : board.first_enabled_at ? "Resume board" : "Publish board"}</button>
        {notice ? <span className="success-text">{notice}</span> : null}
      </div>
      {boardKind(board.board_type) === "announcements" ? <AnnouncementComposer tenant={tenant} board={board} available={board.is_enabled && workspacePublished} /> : null}
      <div className="board-danger-zone">
        {!deleteOpen ? <button type="button" className="ghost-button button--danger" disabled={busy} onClick={() => void openDelete()}>Delete board</button> : <>
          <h3 className="section-title">Delete {board.name}?</h3>
          {deleteSummary ? <>
            <p className="section-subtitle">This board currently has {deleteSummary.delete_posts_count} posts and {deleteSummary.delete_comments_count} comments. Deleting it removes the board and all its content permanently.</p>
            <label className="manage-label">Type <strong>{board.slug}</strong> to confirm<input className="manage-input" value={confirmSlug} onChange={(event) => setConfirmSlug(event.target.value)} autoComplete="off" /></label>
            <div className="manage-board-links"><button type="button" className="ghost-button button--danger" disabled={busy || confirmSlug !== board.slug} onClick={() => void deleteBoard()}>{busy ? "Deleting…" : "Delete permanently"}</button><button type="button" className="ghost-button" disabled={busy} onClick={() => setDeleteOpen(false)}>Cancel</button></div>
          </> : deleteLoading ? <p className="section-subtitle">Checking board contents…</p> : <button type="button" className="ghost-button" onClick={() => setDeleteOpen(false)}>Cancel</button>}
        </>}
      </div>
      {error ? <p className="error-text">{error}</p> : null}
    </section>
  );
}

function AnnouncementComposer({ tenant, board, available }: { tenant: string; board: ManageBoard; available: boolean }) {
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [categories, setCategories] = useState<BoardCategory[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [categoryId, setCategoryId] = useState("");
  const [tagIds, setTagIds] = useState<string[]>([]);

  useEffect(() => {
    if (!available) return;
    void Promise.all([getBoardCategories(tenant, board.slug), getTags(tenant)])
      .then(([nextCategories, nextTags]) => { setCategories(nextCategories); setTags(nextTags); })
      .catch(() => { setCategories([]); setTags([]); });
  }, [available, tenant, board.slug]);

  if (!available) return <p className="section-subtitle">Publish the workspace and this board before posting an announcement.</p>;

  return <form className="manage-fields announcement-composer" onSubmit={async (event) => {
    event.preventDefault();
    if (!title.trim() || !body.trim()) return;
    setBusy(true); setMessage(null);
    try {
      const result = await createPost({ tenantSlug: tenant, boardSlug: board.slug, title: title.trim(), body: body.trim(), categoryId: categoryId || null, tagIds, token: readStoredBearerToken(tenant).trim() });
      setTitle(""); setBody("");
      setCategoryId(""); setTagIds([]);
      setMessage(result.review_state === "pending" ? "Announcement sent for review." : "Announcement published. Open the board to view it.");
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : "Could not publish announcement.");
    } finally { setBusy(false); }
  }}>
    <h3 className="section-title">New announcement</h3>
    <label className="manage-label">Title<input className="manage-input" maxLength={160} value={title} onChange={(event) => setTitle(event.target.value)} required /></label>
    <label className="manage-label">Update<textarea className="manage-input" rows={5} value={body} onChange={(event) => setBody(event.target.value)} required /></label>
    {categories.length ? <label className="manage-label">Category<select className="manage-input" value={categoryId} onChange={(event) => setCategoryId(event.target.value)}><option value="">No category</option>{categories.map((category) => <option key={category.id} value={category.id}>{category.name}</option>)}</select></label> : null}
    {tags.length ? <div className="manage-label">Tags (up to 3)<div className="board-taxonomy__chips">{tags.map((tag) => <label className="manage-check" key={tag.id}><input type="checkbox" checked={tagIds.includes(tag.id)} disabled={!tagIds.includes(tag.id) && tagIds.length >= 3} onChange={(event) => setTagIds((current) => event.target.checked ? [...current, tag.id] : current.filter((id) => id !== tag.id))} />#{tag.name}</label>)}</div></div> : null}
    <button className="button" type="submit" disabled={busy || !title.trim() || !body.trim()}>{busy ? "Publishing…" : "Publish announcement"}</button>
    {message ? <p role="status" className="section-subtitle">{message}</p> : null}
  </form>;
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
