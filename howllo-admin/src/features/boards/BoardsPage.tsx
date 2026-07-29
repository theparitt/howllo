import { useEffect, useRef, useState, type CSSProperties } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { admin } from "@howllo/api-client";
import { adminRoutes } from "@howllo/config";
import type { BoardDetail, BoardSummary, DashboardSection } from "@howllo/types";
import { DASHBOARD_SECTIONS } from "@howllo/types";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";
import { Panel } from "../../components/Panel";
import { SessionRequired } from "../../components/SessionRequired";
import { StateBlock } from "../../components/StateBlock";
import { BoardIcon } from "../../components/BoardIcon";
import { humanizeKey } from "../../lib/format";

const BOARD_TYPES = [
  "feature-requests",
  "bug-reports",
  "discussions",
  "announcements",
];

// Public-facing labels for the dashboard sections an admin can toggle.
const SECTION_LABELS: Record<DashboardSection, { title: string; hint: string }> = {
  progress: { title: "Progress", hint: "Planned / in progress / done columns" },
  latest: { title: "Latest", hint: "Most recent posts" },
  top: { title: "Top requests", hint: "Most-voted posts" },
};

// Map a post status to a badge tone (aligns with howllo-web status colors).
function statusTone(status: string): "green" | "blue" | "warm" | "violet" {
  const s = status.toLowerCase();
  if (s === "done" || s === "complete" || s === "completed") return "green";
  if (s === "in_progress" || s === "in-progress" || s === "planned") return "blue";
  if (s === "declined" || s === "closed") return "warm";
  return "violet";
}

function colorPickerValue(value: string | null | undefined, fallback: string) {
  const trimmed = value?.trim() ?? "";
  return /^#[0-9a-fA-F]{6}$/.test(trimmed) ? trimmed : fallback;
}

function ColorPreviewField({
  label,
  value,
  fallback,
  hint,
  onChange,
}: {
  label: string;
  value: string | null | undefined;
  fallback: string;
  hint: string;
  onChange: (value: string) => void;
}) {
  const resolved = colorPickerValue(value, fallback);
  const style = {
    "--swatch-color": resolved,
  } as CSSProperties;

  return (
    <div className="color-swatch-field">
      <span className="field-label">{label}</span>
      <label className="color-swatch color-swatch--background" style={style}>
        <input
          className="color-swatch__input"
          type="color"
          value={resolved}
          onChange={(event) => onChange(event.target.value)}
          aria-label={`Pick ${label.toLowerCase()}`}
        />
        <span className="color-swatch__surface">
          <span className="color-swatch__chip" aria-hidden="true" />
          <span className="color-swatch__meta">
            <strong>{resolved}</strong>
            <small>{hint}</small>
          </span>
        </span>
      </label>
    </div>
  );
}

export function BoardsPage() {
  const navigate = useNavigate();
  const { boardId } = useParams();
  const { client, tenant, authorization } = useSession();
  const api = admin(client);
  const [selectedBoardId, setSelectedBoardId] = useState<string>("");
  const boards = useAsync<BoardDetail[]>(
    () => (authorization ? api.listBoards() : Promise.resolve([])),
    [tenant, authorization],
  );
  const summary = useAsync<BoardSummary | null>(
    () =>
      authorization && selectedBoardId
        ? api.getBoardSummary(selectedBoardId)
        : Promise.resolve(null),
    [tenant, authorization, selectedBoardId],
  );

  useEffect(() => {
    const data = boards.data;
    if (!data?.length) {
      setSelectedBoardId("");
      return;
    }

    const hasBoard = (id: string) => data.some((board) => board.id === id);
    if (boardId) {
      if (hasBoard(boardId)) {
        if (selectedBoardId !== boardId) {
          setSelectedBoardId(boardId);
        }
        return;
      }

      const fallbackId = data[0]!.id;
      setSelectedBoardId(fallbackId);
      navigate(adminRoutes.board(fallbackId), { replace: true });
      return;
    }

    if (!selectedBoardId || !hasBoard(selectedBoardId)) {
      setSelectedBoardId(data[0]!.id);
    }
  }, [boardId, boards.data, navigate, selectedBoardId]);

  if (!authorization) {
    return (
      <section>
        <h1>Boards</h1>
        <SessionRequired />
      </section>
    );
  }

  const activeBoard =
    boards.data?.find((board) => board.id === selectedBoardId) ?? boards.data?.[0] ?? null;

  const selectBoard = (id: string) => {
    setSelectedBoardId(id);
    navigate(adminRoutes.board(id));
  };

  return (
    <section>
      <h1>Boards</h1>
      <p className="muted">
        Configure board identity, privacy, and structure for{" "}
        <strong>{tenant || "-"}</strong> workspace.
      </p>

      <CreateBoard onCreated={boards.reload} />

      <div className="content-grid">
        <Panel title="All boards">
          <StateBlock
            loading={boards.loading}
            error={boards.error}
            empty={boards.data?.length === 0 ? "No boards yet." : null}
          >
            <div className="stack-list">
              {boards.data?.map((board) => (
                <button
                  key={board.id}
                  className={
                    board.id === activeBoard?.id ? "stack-item active" : "stack-item"
                  }
                  onClick={() => selectBoard(board.id)}
                >
                  <div className="board-row">
                    <BoardIcon iconUrl={board.icon_url} size={38} />
                    <div>
                      <strong>{board.name}</strong>
                      <div className="muted">
                        <code>{board.slug}</code> - {humanizeKey(board.board_type)}
                      </div>
                    </div>
                  </div>
                  <span
                    className="badge"
                    data-tone={board.is_private ? "warm" : "green"}
                  >
                    {board.is_private ? "Private" : "Public"}
                  </span>
                </button>
              ))}
            </div>
          </StateBlock>
        </Panel>

        <Panel title={activeBoard ? `Board settings - ${activeBoard.name}` : "Board settings"}>
          <StateBlock
            loading={boards.loading}
            error={boards.error}
            empty={!activeBoard ? "Select a board to edit it." : null}
          >
            {activeBoard ? (
              <EditBoard
                board={activeBoard}
                summary={summary}
                onChanged={() => {
                  boards.reload();
                  summary.reload();
                }}
              />
            ) : null}
          </StateBlock>
        </Panel>
      </div>
    </section>
  );
}

function CreateBoard({ onCreated }: { onCreated: () => void }) {
  const { client } = useSession();
  const api = admin(client);
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [slugTouched, setSlugTouched] = useState(false);
  const [description, setDescription] = useState("");
  const [boardType, setBoardType] = useState(BOARD_TYPES[0]!);
  const [isPrivate, setIsPrivate] = useState(false);
  const [iconUrl, setIconUrl] = useState<string | null>(null);
  const [backgroundColor, setBackgroundColor] = useState("#fff1ea");
  const [iconUploading, setIconUploading] = useState(false);
  const iconInputRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (slugTouched) return;
    const next = name
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
    setSlug(next);
  }, [name, slugTouched]);

  const onPickIcon = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    if (!file.type.startsWith("image/")) {
      setError("Please choose an image file.");
      return;
    }
    setIconUploading(true);
    setError(null);
    try {
      setIconUrl(await client.uploadImage(file));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Icon upload failed.");
    } finally {
      setIconUploading(false);
    }
  };

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.createBoard({
        name: name.trim(),
        slug: slug.trim(),
        description: description.trim() || undefined,
        board_type: boardType,
        is_private: isPrivate,
        icon_url: iconUrl,
        background_color: backgroundColor || undefined,
      });
      setName("");
      setSlug("");
      setSlugTouched(false);
      setDescription("");
      setIconUrl(null);
      setBackgroundColor("#fff1ea");
      onCreated();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create board");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel title="New board">
      <div className="detail-stack">
        <div className="board-icon-field">
          <BoardIcon iconUrl={iconUrl} size={52} />
          <div className="board-icon-field__actions">
            <span className="field-label">Board icon</span>
            <div className="button-row">
              <input
                ref={iconInputRef}
                type="file"
                accept="image/*"
                hidden
                onChange={onPickIcon}
              />
              <button type="button" disabled={iconUploading} onClick={() => iconInputRef.current?.click()}>
                {iconUploading ? "Uploading..." : iconUrl ? "Replace" : "Upload icon"}
              </button>
              {iconUrl ? (
                <button type="button" className="danger" onClick={() => setIconUrl(null)}>
                  Remove
                </button>
              ) : null}
            </div>
          </div>
        </div>
        <div className="form-grid">
          <label>
            Display name
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Feature Requests"
            />
          </label>
          <label>
            Slug
            <input
              value={slug}
              onChange={(e) => {
                setSlugTouched(true);
                setSlug(e.target.value);
              }}
              placeholder="feature-requests"
            />
          </label>
          <label>
            Type
            <select
              value={boardType}
              onChange={(e) => setBoardType(e.target.value)}
            >
              {BOARD_TYPES.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
          </label>
          <label className="checkbox toggle-field">
            <input
              type="checkbox"
              checked={isPrivate}
              onChange={(e) => setIsPrivate(e.target.checked)}
            />
            <span>
              Private board
              <span className="toggle-field__hint">Only members can view it</span>
            </span>
          </label>
          <label>
            Board background
            <input
              value={backgroundColor}
              onChange={(e) => setBackgroundColor(e.target.value)}
              placeholder="#fff1ea"
            />
          </label>
          <ColorPreviewField
            label="Background preview"
            value={backgroundColor}
            fallback="#fff1ea"
            hint="Click to choose the board backdrop"
            onChange={setBackgroundColor}
          />
          <label className="field-stack field-span-2">
            Description
            <textarea
              rows={3}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Explain what kind of feedback belongs on this board."
            />
          </label>
        </div>
        <div className="button-row">
          <button
            className="primary"
            disabled={busy || !name.trim() || !slug.trim()}
            onClick={submit}
          >
            {busy ? "Creating..." : "Create"}
          </button>
          <span className="muted small">
            Slug auto-fills from the display name until you edit it manually.
          </span>
        </div>
      </div>
      {error ? <p className="error-text">{error}</p> : null}
    </Panel>
  );
}

function EditBoard({
  board,
  summary,
  onChanged,
}: {
  board: BoardDetail;
  summary: {
    data: BoardSummary | null;
    error: string | null;
    loading: boolean;
    reload: () => void;
  };
  onChanged: () => void;
}) {
  const { client } = useSession();
  const api = admin(client);
  const [name, setName] = useState(board.name);
  const [description, setDescription] = useState(board.description ?? "");
  const [boardType, setBoardType] = useState(board.board_type);
  const [isPrivate, setIsPrivate] = useState(board.is_private);
  const [iconUrl, setIconUrl] = useState<string | null>(board.icon_url);
  const [backgroundColor, setBackgroundColor] = useState(board.background_color ?? "#fff1ea");
  const [dashboardSections, setDashboardSections] = useState<DashboardSection[]>(
    board.dashboard_sections ?? DASHBOARD_SECTIONS,
  );
  const [iconUploading, setIconUploading] = useState(false);
  const iconInputRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setName(board.name);
    setDescription(board.description ?? "");
    setBoardType(board.board_type);
    setIsPrivate(board.is_private);
    setIconUrl(board.icon_url);
    setBackgroundColor(board.background_color ?? "#fff1ea");
    setDashboardSections(board.dashboard_sections ?? DASHBOARD_SECTIONS);
    setError(null);
  }, [board]);

  const toggleSection = (section: DashboardSection) => {
    setDashboardSections((current) =>
      current.includes(section)
        ? current.filter((s) => s !== section)
        : DASHBOARD_SECTIONS.filter((s) => s === section || current.includes(s)),
    );
  };

  const onPickIcon = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    if (!file.type.startsWith("image/")) {
      setError("Please choose an image file.");
      return;
    }
    setIconUploading(true);
    setError(null);
    try {
      setIconUrl(await client.uploadImage(file));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Icon upload failed.");
    } finally {
      setIconUploading(false);
    }
  };

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.updateBoard(board.id, {
        name: name.trim(),
        description: description.trim() || undefined,
        board_type: boardType,
        is_private: isPrivate,
        icon_url: iconUrl,
        background_color: backgroundColor || undefined,
        dashboard_sections: dashboardSections,
      });
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to update board");
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    const impact = summary.data?.total_posts ?? 0;
    const message =
      impact > 0
        ? `Delete board "${board.name}" and ${impact} posts under it? This cannot be undone.`
        : `Delete empty board "${board.name}"? This cannot be undone.`;
    const confirmed = window.confirm(message);
    if (!confirmed) return;

    setBusy(true);
    setError(null);
    try {
      await api.deleteBoard(board.id);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to delete board");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="detail-stack">
      <fieldset className="field-group">
        <legend className="field-group__title">Basics</legend>

        <div className="board-icon-field">
          <BoardIcon iconUrl={iconUrl} size={64} />
          <div className="board-icon-field__actions">
            <span className="field-label">Board icon</span>
            <div className="button-row">
              <input
                ref={iconInputRef}
                type="file"
                accept="image/*"
                hidden
                onChange={onPickIcon}
              />
              <button
                type="button"
                disabled={iconUploading}
                onClick={() => iconInputRef.current?.click()}
              >
                {iconUploading ? "Uploading..." : iconUrl ? "Replace" : "Upload icon"}
              </button>
              {iconUrl ? (
                <button type="button" className="danger" onClick={() => setIconUrl(null)}>
                  Remove
                </button>
              ) : null}
            </div>
            <p className="muted small">
              {iconUrl
                ? "Custom icon. Remove to use the Howllo default."
                : "Using the Howllo default icon. Upload one to override it."}
            </p>
          </div>
        </div>

        <div className="form-grid">
          <label>
            Display name
            <input value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label>
            Slug
            <input value={board.slug} disabled />
          </label>
        </div>

        <label className="field-stack">
          Description
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={4}
          />
        </label>
      </fieldset>

      <fieldset className="field-group">
        <legend className="field-group__title">Access &amp; type</legend>
        <div className="form-grid">
          <label>
            Type
            <select value={boardType} onChange={(e) => setBoardType(e.target.value)}>
              {BOARD_TYPES.map((type) => (
                <option key={type} value={type}>
                  {type}
                </option>
              ))}
            </select>
          </label>
          <label className="checkbox toggle-field">
            <input
              type="checkbox"
              checked={isPrivate}
              onChange={(e) => setIsPrivate(e.target.checked)}
            />
            <span>
              Private board
              <span className="toggle-field__hint">Only members can view it</span>
            </span>
          </label>
        </div>
      </fieldset>

      <fieldset className="field-group">
        <legend className="field-group__title">Appearance</legend>
        <div className="form-grid board-appearance-grid">
          <label>
            Board background
            <input
              value={backgroundColor}
              onChange={(e) => setBackgroundColor(e.target.value)}
              placeholder="#fff1ea"
            />
          </label>
          <ColorPreviewField
            label="Background preview"
            value={backgroundColor}
            fallback="#fff1ea"
            hint="Click to choose the board backdrop"
            onChange={setBackgroundColor}
          />
        </div>
      </fieldset>

      <fieldset className="field-group">
        <legend className="field-group__title">Dashboard sections</legend>
        <p className="field-group__hint">
          Choose which blocks appear on this board&rsquo;s public dashboard. Turn off the
          noisy ones to keep it focused.
        </p>
        <div className="section-toggles">
          {DASHBOARD_SECTIONS.map((section) => (
            <label key={section} className="checkbox toggle-field">
              <input
                type="checkbox"
                checked={dashboardSections.includes(section)}
                onChange={() => toggleSection(section)}
              />
              <span>
                {SECTION_LABELS[section].title}
                <span className="toggle-field__hint">{SECTION_LABELS[section].hint}</span>
              </span>
            </label>
          ))}
        </div>
      </fieldset>

      <div className="button-row">
        <button className="primary" disabled={busy || !name.trim()} onClick={save}>
          {busy ? "Saving..." : "Save changes"}
        </button>
        <button className="danger" disabled={busy} onClick={remove}>
          Delete board
        </button>
      </div>

      {error ? <p className="error-text">{error}</p> : null}

      <Panel title="Board summary">
        <StateBlock
          loading={summary.loading}
          error={summary.error}
          empty={!summary.data ? "No summary available yet." : null}
        >
          {summary.data ? (
            <div className="summary-grid">
              <div className="metric-card">
                <span>Total posts</span>
                <strong>{summary.data.total_posts}</strong>
              </div>
              <div className="metric-card">
                <span>Total votes</span>
                <strong>{summary.data.total_votes}</strong>
              </div>
              <div className="metric-card">
                <span>Total comments</span>
                <strong>{summary.data.total_comments}</strong>
              </div>
              <div className="metric-card field-span-3">
                <span>Status mix</span>
                <div className="chip-row">
                  {Object.entries(summary.data.posts_by_status).map(([status, count]) => (
                    <span key={status} className="badge" data-tone={statusTone(status)}>
                      {humanizeKey(status)} - {count}
                    </span>
                  ))}
                </div>
              </div>
            </div>
          ) : null}
        </StateBlock>
      </Panel>
    </div>
  );
}
