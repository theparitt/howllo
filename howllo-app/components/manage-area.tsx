"use client";

import { useCallback, useEffect, useState } from "react";
import { RooiamInlineLogin } from "@/components/rooiam-inline-login";
import { ENABLED_AUTH_PROVIDERS } from "@/lib/auth-provider";
import {
  disableSso,
  getMyWorkspaceRole,
  getTenantManagementSettings,
  getSsoConfig,
  regenerateSsoSecret,
  type SsoConfig,
  managerCreateInvitation,
  managerCreateBoard,
  managerDeleteBoard,
  managerGetBoardSummary,
  managerListBoards,
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
import type { ManageBoard, MyInvitation, WorkspaceMember } from "@/lib/types";
import { API_BASE_URL } from "@/lib/config";
import { uploadImage } from "@/lib/api";
import { ParticipantsTab } from "./participants-tab";
import { ModerationTab } from "./moderation-tab";
import { BrandingTab } from "./branding-tab";
import { SecurityTab } from "./security-tab";
import { prepareBrandImage } from "../lib/prepare-brand-image";

const INVITE_ROLES = ["admin", "moderator"];
const MEMBER_ROLES = ["owner", "admin", "moderator", "member"];
const PUBLIC_WEB_URL = (process.env.NEXT_PUBLIC_HOWLLO_PUBLIC_WEB_URL || "http://localhost:7703").replace(/\/$/, "");

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
  const [tab, setTab] = useState<"team" | "boards" | "branding" | "security" | "signin" | "participants" | "moderation">("boards");

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

  useEffect(() => {
    if (role === "moderator") setTab("moderation");
  }, [role]);

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

  return (
    <div className="page-stack">
      <section className="page-head">
        <h1 className="page-title">Manage workspace</h1>
        <p className="page-lead">Create boards, choose who can see them, and share public board links with your users.</p>
      </section>

      <div className="feed-filters">
        {role !== "moderator" ? <>
        <button type="button" className={`feed-chip${tab === "team" ? " feed-chip--active" : ""}`} onClick={() => setTab("team")}>
          Staff
        </button>
        <button type="button" className={`feed-chip${tab === "boards" ? " feed-chip--active" : ""}`} onClick={() => setTab("boards")}>
          Boards
        </button>
        <button type="button" className={`feed-chip${tab === "branding" ? " feed-chip--active" : ""}`} onClick={() => setTab("branding")}>Public site</button>
        <button type="button" className={`feed-chip${tab === "security" ? " feed-chip--active" : ""}`} onClick={() => setTab("security")}>Security</button>
        <button type="button" className={`feed-chip${tab === "participants" ? " feed-chip--active" : ""}`} onClick={() => setTab("participants")}>Users</button>
        <button type="button" className={`feed-chip${tab === "signin" ? " feed-chip--active" : ""}`} onClick={() => setTab("signin")}>
          Sign-in (SSO)
        </button>
        </> : null}
        <button type="button" className={`feed-chip${tab === "moderation" ? " feed-chip--active" : ""}`} onClick={() => setTab("moderation")}>Moderation</button>
      </div>

      {tenant ? (
        tab === "moderation" || role === "moderator" ? (
          <ModerationTab tenant={tenant} />
        ) : tab === "participants" ? (
          <ParticipantsTab tenant={tenant} />
        ) : tab === "branding" ? (
          <BrandingTab tenant={tenant} />
        ) : tab === "security" ? (
          <SecurityTab tenant={tenant} />
        ) : tab === "team" ? (
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
      setWorkspacePublished(settings.is_published);
      setSelected((cur) => (cur && list.some((item) => item.id === cur) ? cur : list[0]?.id ?? null));
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not load boards.");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tenant]);

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

  return (
    <div className="page-stack">
      <section className="panel">
        <div className="manage-board-heading">
          <div>
            <h2 className="section-title">Your boards</h2>
            <p className="section-subtitle">Create boards one at a time. Each starts as a draft until you publish it.</p>
          </div>
          <button className="button button--cta" type="button" onClick={() => setCreating((value) => !value)}>{creating ? "Cancel" : "Create board"}</button>
        </div>
        {creating ? <form className="manage-fields" style={{ marginTop: "1rem" }} onSubmit={(event) => void createBoard(event)}>
          <label className="manage-label">Board name<input className="manage-input" value={name} onChange={(event) => setName(event.target.value)} placeholder="Product ideas" required /></label>
          <label className="manage-label">URL slug<input className="manage-input" value={slugInput} onChange={(event) => setSlugInput(event.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ""))} placeholder={generatedSlug || "product-ideas"} pattern="[a-z0-9]+(?:-[a-z0-9]+)*" required={!generatedSlug} /></label>
          <label className="manage-label">Description<textarea className="manage-input" rows={2} value={description} onChange={(event) => setDescription(event.target.value)} /></label>
          <div className="manage-label">Board type<BoardTypePicker value={boardType} onChange={setBoardType} group="create-board-kind" /></div>
          <label className="manage-check"><input type="checkbox" checked={isPrivate} onChange={(event) => setIsPrivate(event.target.checked)} /> Private board (members only)</label>
          <button className="button button--cta" type="submit" disabled={busy || !name.trim() || !slug}>{busy ? "Creating…" : "Create board"}</button>
        </form> : null}
        {error ? <p className="error-text" role="alert">{error}</p> : null}
      </section>
      <section className="dashboard-grid">
      <section className="panel">
        <h2 className="section-title">Boards</h2>
        <div className="list-stack" style={{ marginTop: "1rem" }}>
          {boards.length === 0 ? <p className="section-subtitle">No boards yet. Create your first board above.</p> : null}
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
      </section>
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
        }}>Copy public link</button> : <span className="section-subtitle">{!board.is_enabled ? "Hidden from visitors" : !workspacePublished ? "Publish this workspace in Public site to share its boards." : "Private: workspace members only"}</span>}
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

  if (!available) return <p className="section-subtitle">Publish the workspace and this board before posting an announcement.</p>;

  return <form className="manage-fields announcement-composer" onSubmit={async (event) => {
    event.preventDefault();
    if (!title.trim() || !body.trim()) return;
    setBusy(true); setMessage(null);
    try {
      const result = await createPost({ tenantSlug: tenant, boardSlug: board.slug, title: title.trim(), body: body.trim(), token: readStoredBearerToken(tenant).trim() });
      setTitle(""); setBody("");
      setMessage(result.review_state === "pending" ? "Announcement sent for review." : "Announcement published. Open the board to view it.");
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : "Could not publish announcement.");
    } finally { setBusy(false); }
  }}>
    <h3 className="section-title">New announcement</h3>
    <label className="manage-label">Title<input className="manage-input" maxLength={160} value={title} onChange={(event) => setTitle(event.target.value)} required /></label>
    <label className="manage-label">Update<textarea className="manage-input" rows={5} value={body} onChange={(event) => setBody(event.target.value)} required /></label>
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
