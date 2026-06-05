import { useMemo } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  BOARD_WEB_BASE_URL,
  adminRoutes,
  publicRoutes,
} from "@howllo/config";
import { admin, HowlloClient, roadmap } from "@howllo/api-client";
import type {
  AdminTenantSummary,
  BoardDetail,
  MembershipItem,
  RoadmapItem,
  TenantBranding,
} from "@howllo/types";
import { Panel } from "../../components/Panel";
import { SessionRequired } from "../../components/SessionRequired";
import { StateBlock } from "../../components/StateBlock";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";
import { formatDateTime } from "../../lib/format";

export function WorkspaceDetailPage() {
  const navigate = useNavigate();
  const { workspaceSlug = "" } = useParams();
  const { client, authorization, tenant, setTenant } = useSession();

  const workspaceClient = useMemo(
    () =>
      authorization
        ? new HowlloClient({
            baseUrl: client.baseUrl,
            authorization,
            tenant: workspaceSlug,
          })
        : null,
    [authorization, client.baseUrl, workspaceSlug],
  );

  const platformApi = admin(client);
  const workspaceApi = workspaceClient ? admin(workspaceClient) : null;
  const workspaceRoadmap = workspaceClient ? roadmap(workspaceClient) : null;

  const workspaces = useAsync<AdminTenantSummary[]>(
    () => (authorization ? platformApi.listTenants() : Promise.resolve([])),
    [authorization],
  );
  const boards = useAsync<BoardDetail[]>(
    () =>
      authorization && workspaceApi
        ? workspaceApi.listBoards()
        : Promise.resolve([]),
    [authorization, workspaceSlug],
  );
  const members = useAsync<MembershipItem[]>(
    () =>
      authorization && workspaceApi
        ? workspaceApi.listMembers()
        : Promise.resolve([]),
    [authorization, workspaceSlug],
  );
  const branding = useAsync<TenantBranding | null>(
    () =>
      authorization && workspaceApi
        ? workspaceApi.getTenantBranding()
        : Promise.resolve(null),
    [authorization, workspaceSlug],
  );
  const roadmapItems = useAsync<RoadmapItem[]>(
    () =>
      authorization && workspaceRoadmap
        ? workspaceRoadmap.list()
        : Promise.resolve([]),
    [authorization, workspaceSlug],
  );

  if (!authorization) {
    return (
      <section>
        <h1>Workspace details</h1>
        <SessionRequired />
      </section>
    );
  }

  const workspace = workspaces.data?.find((item) => item.slug === workspaceSlug) ?? null;
  const publicBoard =
    boards.data?.find((board) => !board.is_private) ?? null;
  const publicBoardUrl =
    publicBoard && workspaceSlug
      ? `${BOARD_WEB_BASE_URL}${publicRoutes.board(workspaceSlug, publicBoard.slug)}`
      : null;
  const publicRoadmapUrl = workspaceSlug
    ? `${BOARD_WEB_BASE_URL}${publicRoutes.roadmap(workspaceSlug)}`
    : null;

  const setContext = () => {
    setTenant(workspaceSlug);
  };

  const openWorkspaceTool = (route: string) => {
    setTenant(workspaceSlug);
    navigate(route);
  };

  return (
    <section>
      <div className="page-head">
        <div>
          <h1>Workspace details</h1>
          <p className="muted">
            Platform-level overview for <strong>{workspace?.name ?? workspaceSlug}</strong>.
          </p>
        </div>
        <div className="button-row">
          <button onClick={() => navigate(adminRoutes.tenants())}>Back</button>
          <button
            className="primary"
            disabled={!workspaceSlug || tenant === workspaceSlug}
            onClick={setContext}
          >
            {tenant === workspaceSlug ? "Current workspace" : "Set context"}
          </button>
        </div>
      </div>

      <StateBlock
        loading={workspaces.loading}
        error={workspaces.error}
        empty={!workspace ? `Workspace ${workspaceSlug} was not found.` : null}
      >
        {workspace ? (
          <>
            <div className="summary-grid">
              <Panel title="Boards">
                <div className="stat-number">{workspace.board_count}</div>
              </Panel>
              <Panel title="Members">
                <div className="stat-number">{workspace.member_count}</div>
              </Panel>
              <Panel title="Roadmap items">
                <StateBlock loading={roadmapItems.loading} error={roadmapItems.error}>
                  <div className="stat-number">{roadmapItems.data?.length ?? 0}</div>
                </StateBlock>
              </Panel>
            </div>

            <div className="content-grid">
              <Panel title="Overview">
                <dl className="detail-list">
                  <div>
                    <dt>Name</dt>
                    <dd>{workspace.name}</dd>
                  </div>
                  <div>
                    <dt>Slug</dt>
                    <dd>
                      <code>{workspace.slug}</code>
                    </dd>
                  </div>
                  <div>
                    <dt>Created</dt>
                    <dd>{formatDateTime(workspace.created_at)}</dd>
                  </div>
                  <div>
                    <dt>Context</dt>
                    <dd>{tenant === workspace.slug ? "Current workspace" : "Not selected"}</dd>
                  </div>
                </dl>
              </Panel>

              <Panel title="Public links">
                <div className="detail-stack">
                  <div className="button-row">
                    {publicBoardUrl ? (
                      <a href={publicBoardUrl} target="_blank" rel="noreferrer">
                        Open public board
                      </a>
                    ) : (
                      <button disabled>No public board yet</button>
                    )}
                    {publicRoadmapUrl ? (
                      <a href={publicRoadmapUrl} target="_blank" rel="noreferrer">
                        Open roadmap
                      </a>
                    ) : null}
                  </div>
                  <p className="muted small">
                    Public web base: <code>{BOARD_WEB_BASE_URL}</code>
                  </p>
                  {publicBoardUrl ? (
                    <p className="muted small">
                      Public board URL: <code>{publicBoardUrl}</code>
                    </p>
                  ) : null}
                </div>
              </Panel>
            </div>

            <div className="content-grid">
              <Panel title="Branding">
                <StateBlock
                  loading={branding.loading}
                  error={branding.error}
                  empty={!branding.data ? "No branding loaded." : null}
                >
                  {branding.data ? (
                    <div className="detail-stack">
                      <div className="logo-preview logo-preview--inline">
                        {branding.data.logo_url ? (
                          <img src={branding.data.logo_url} alt={branding.data.site_name} />
                        ) : (
                          <span className="logo-preview__empty">No logo</span>
                        )}
                      </div>
                      <dl className="detail-list">
                        <div>
                          <dt>Public site name</dt>
                          <dd>{branding.data.site_name}</dd>
                        </div>
                        <div>
                          <dt>Accent color</dt>
                          <dd>
                            <code>{branding.data.accent_color || "-"}</code>
                          </dd>
                        </div>
                        <div>
                          <dt>Powered by Howllo</dt>
                          <dd>{branding.data.show_powered_by ? "Shown" : "Hidden"}</dd>
                        </div>
                      </dl>
                      <div className="button-row">
                        <button onClick={() => openWorkspaceTool(adminRoutes.settings())}>
                          Open settings
                        </button>
                      </div>
                    </div>
                  ) : null}
                </StateBlock>
              </Panel>

              <Panel title="Workspace tools">
                <div className="stack-list">
                  <button
                    className="stack-item"
                    onClick={() => openWorkspaceTool(adminRoutes.boards())}
                  >
                    <strong>Boards</strong>
                    <div className="muted">Manage board structure and visibility.</div>
                  </button>
                  <button
                    className="stack-item"
                    onClick={() => openWorkspaceTool(adminRoutes.members())}
                  >
                    <strong>Members</strong>
                    <div className="muted">Manage access and role boundaries.</div>
                  </button>
                  <button
                    className="stack-item"
                    onClick={() => openWorkspaceTool(adminRoutes.roadmap())}
                  >
                    <strong>Roadmap</strong>
                    <div className="muted">Create and move roadmap items.</div>
                  </button>
                  <button
                    className="stack-item"
                    onClick={() => openWorkspaceTool(adminRoutes.integrations())}
                  >
                    <strong>Integrations</strong>
                    <div className="muted">Manage webhooks and API tokens.</div>
                  </button>
                </div>
              </Panel>
            </div>

            <div className="content-grid">
              <Panel title="Boards in this workspace">
                <StateBlock
                  loading={boards.loading}
                  error={boards.error}
                  empty={boards.data?.length === 0 ? "No boards yet." : null}
                >
                  <table className="data-table">
                    <thead>
                      <tr>
                        <th>Name</th>
                        <th>Slug</th>
                        <th>Type</th>
                        <th>Visibility</th>
                      </tr>
                    </thead>
                    <tbody>
                      {boards.data?.map((board) => (
                        <tr key={board.id}>
                          <td>{board.name}</td>
                          <td>
                            <code>{board.slug}</code>
                          </td>
                          <td>{board.board_type}</td>
                          <td>{board.is_private ? "Private" : "Public"}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </StateBlock>
              </Panel>

              <Panel title="Members in this workspace">
                <StateBlock
                  loading={members.loading}
                  error={members.error}
                  empty={members.data?.length === 0 ? "No members yet." : null}
                >
                  <table className="data-table">
                    <thead>
                      <tr>
                        <th>Name</th>
                        <th>Email</th>
                        <th>Role</th>
                      </tr>
                    </thead>
                    <tbody>
                      {members.data?.map((member) => (
                        <tr key={member.user_id}>
                          <td>{member.display_name}</td>
                          <td>{member.email}</td>
                          <td>{member.role}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </StateBlock>
              </Panel>
            </div>
          </>
        ) : null}
      </StateBlock>
    </section>
  );
}
