import { useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { adminRoutes } from "@howllo/config";
import { admin } from "@howllo/api-client";
import type { AdminTenantSummary } from "@howllo/types";
import { Panel } from "../../components/Panel";
import { SessionRequired } from "../../components/SessionRequired";
import { StateBlock } from "../../components/StateBlock";
import { formatDateTime } from "../../lib/format";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";
import { useEscapeKey, useFocusTrap } from "../../lib/a11y";

export function TenantsPage() {
  const navigate = useNavigate();
  const { client, tenant, authorization, setTenant } = useSession();
  const api = admin(client);
  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [toDelete, setToDelete] = useState<AdminTenantSummary | null>(null);
  const tenants = useAsync<AdminTenantSummary[]>(
    () => (authorization ? api.listTenants() : Promise.resolve([])),
    [authorization],
  );

  if (!authorization) {
    return (
      <section>
        <h1>Workspaces</h1>
        <SessionRequired />
      </section>
    );
  }

  const openTenant = (slug: string) => {
    setTenant(slug);
    navigate(adminRoutes.boards());
  };

  const openWorkspaceDetail = (slug: string) => {
    navigate(adminRoutes.tenantDetail(slug));
  };

  const createWorkspace = async () => {
    setCreating(true);
    setCreateError(null);
    try {
      const item = await api.createTenant({
        name: name.trim(),
      });
      setName("");
      tenants.reload();
      setTenant(item.slug);
      navigate(adminRoutes.boards());
    } catch (error) {
      setCreateError(
        error instanceof Error ? error.message : "Failed to create workspace",
      );
    } finally {
      setCreating(false);
    }
  };

  // The oldest workspace is the platform default and cannot be deleted.
  const defaultId = tenants.data?.[0]?.id ?? null;

  return (
    <section>
      <h1>Workspaces</h1>
      <p className="muted">
        System-level workspace inventory. Choose a workspace context, then use
        the workspace tools for boards, members, branding, and moderation.
      </p>

      <Panel title="Create workspace">
        <div className="form-row">
          <label>
            Name
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Howllo"
            />
          </label>
          <button
            className="primary"
            disabled={creating || !name.trim()}
            onClick={createWorkspace}
          >
            {creating ? "Creating..." : "Create workspace"}
          </button>
        </div>
        <p className="muted small">
          Workspace slugs are generated automatically from the name, using
          <code> name-random4or5</code>.
        </p>
        {createError ? <p className="error-text">{createError}</p> : null}
      </Panel>

      <Panel title="All workspaces">
        <StateBlock
          loading={tenants.loading}
          error={tenants.error}
          empty={tenants.data?.length === 0 ? "No workspaces exist yet." : null}
        >
          <table className="data-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Slug</th>
                <th>Boards</th>
                <th>Members</th>
                <th>Created</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {tenants.data?.map((item) => (
                <tr key={item.id}>
                  <td>
                    <strong>{item.name}</strong>
                    {tenant === item.slug ? (
                      <div className="badge" data-tone="green">Current workspace</div>
                    ) : null}
                    {item.id === defaultId ? (
                      <div className="badge">Default</div>
                    ) : null}
                  </td>
                  <td>
                    <code>{item.slug}</code>
                  </td>
                  <td>{item.board_count}</td>
                  <td>{item.member_count}</td>
                  <td>{formatDateTime(item.created_at)}</td>
                  <td className="row-actions">
                    <button onClick={() => openWorkspaceDetail(item.slug)}>
                      Details
                    </button>
                    <button
                      className="primary small"
                      onClick={() => openTenant(item.slug)}
                    >
                      Set context
                    </button>
                    {item.id !== defaultId ? (
                      <button className="danger small" onClick={() => setToDelete(item)}>
                        Delete
                      </button>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </StateBlock>
      </Panel>

      {toDelete ? (
        <DeleteWorkspaceModal
          workspace={toDelete}
          onClose={() => setToDelete(null)}
          onDeleted={(slug) => {
            setToDelete(null);
            // If we just deleted the active context, fall back to the default.
            if (tenant === slug && defaultId) {
              const fallback = tenants.data?.find((t) => t.id === defaultId);
              if (fallback) setTenant(fallback.slug);
            }
            tenants.reload();
          }}
        />
      ) : null}
    </section>
  );
}

function DeleteWorkspaceModal({
  workspace,
  onClose,
  onDeleted,
}: {
  workspace: AdminTenantSummary;
  onClose: () => void;
  onDeleted: (slug: string) => void;
}) {
  const { client } = useSession();
  const api = admin(client);
  const dialogRef = useRef<HTMLDivElement>(null);
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEscapeKey(true, onClose);
  useFocusTrap(true, dialogRef);

  const matches = confirm.trim() === workspace.slug;

  const doDelete = async () => {
    if (!matches) return;
    setBusy(true);
    setError(null);
    try {
      await api.deleteTenant(workspace.slug, confirm.trim());
      onDeleted(workspace.slug);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to delete workspace.");
      setBusy(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-card"
        role="dialog"
        aria-modal="true"
        ref={dialogRef}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="modal-title">Delete workspace</h2>
        <p className="muted">
          This permanently deletes <strong>{workspace.name}</strong> and{" "}
          <strong>everything in it</strong> - all boards, posts, comments,
          members, and history. This cannot be undone.
        </p>

        <div className="modal-stats">
          <span>{workspace.board_count} boards</span>
          <span>{workspace.member_count} members</span>
        </div>

        <label className="auth-field" style={{ marginTop: 4 }}>
          <span>
            Type <code>{workspace.slug}</code> to confirm
          </span>
          <input
            value={confirm}
            autoFocus
            onChange={(e) => setConfirm(e.target.value)}
            placeholder={workspace.slug}
          />
        </label>

        {error ? <p className="error-text">{error}</p> : null}

        <div className="modal-actions">
          <button onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button
            className="danger"
            disabled={!matches || busy}
            onClick={doDelete}
          >
            {busy ? "Deleting..." : "Delete workspace"}
          </button>
        </div>
      </div>
    </div>
  );
}
