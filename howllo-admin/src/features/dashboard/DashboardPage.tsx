import { admin, roadmap } from "@howllo/api-client";
import type {
  AdminTenantSummary,
  AuditLogItem,
  BoardDetail,
  MembershipItem,
  PaginatedResponse,
  PlatformStatusResponse,
  RoadmapItem,
} from "@howllo/types";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";
import { Panel } from "../../components/Panel";
import { SessionRequired } from "../../components/SessionRequired";
import { StateBlock } from "../../components/StateBlock";
import { formatDateTime, humanizeKey } from "../../lib/format";

export function DashboardPage() {
  const { client, tenant, authorization } = useSession();
  const api = admin(client);
  const road = roadmap(client);

  const boards = useAsync<BoardDetail[]>(
    () => (authorization ? api.listBoards() : Promise.resolve([])),
    [tenant, authorization],
  );
  const members = useAsync<MembershipItem[]>(
    () => (authorization ? api.listMembers() : Promise.resolve([])),
    [tenant, authorization],
  );
  const roadmapItems = useAsync<RoadmapItem[]>(
    () => (authorization ? road.list() : Promise.resolve([])),
    [tenant, authorization],
  );
  const audit = useAsync<PaginatedResponse<AuditLogItem>>(
    () =>
      authorization
        ? api.listAuditLogs(1, 8)
        : Promise.resolve({
            items: [],
            page: 1,
            per_page: 8,
            total: 0,
            has_next: false,
          }),
    [tenant, authorization],
  );
  const workspaces = useAsync<AdminTenantSummary[]>(
    () => (authorization ? api.listTenants() : Promise.resolve([])),
    [authorization],
  );
  const platformStatus = useAsync<PlatformStatusResponse | null>(
    () => (authorization ? api.getPlatformStatus() : Promise.resolve(null)),
    [authorization],
  );

  if (!authorization) {
    return (
      <section>
        <h1>Dashboard</h1>
        <SessionRequired />
      </section>
    );
  }

  return (
    <section>
      <h1>Dashboard</h1>
      <p className="muted">
        What&rsquo;s happening in <strong>{tenant}</strong>.
      </p>

      <div className="summary-grid">
        <Panel title="Workspaces">
          <StateBlock loading={workspaces.loading} error={workspaces.error}>
            <div className="stat-number">{workspaces.data?.length ?? 0}</div>
          </StateBlock>
        </Panel>
        <Panel title="Boards">
          <StateBlock loading={boards.loading} error={boards.error}>
            <div className="stat-number">{boards.data?.length ?? 0}</div>
          </StateBlock>
        </Panel>
        <Panel title="Members">
          <StateBlock loading={members.loading} error={members.error}>
            <div className="stat-number">{members.data?.length ?? 0}</div>
          </StateBlock>
        </Panel>
        <Panel title="On the roadmap">
          <StateBlock loading={roadmapItems.loading} error={roadmapItems.error}>
            <div className="stat-number">{roadmapItems.data?.length ?? 0}</div>
          </StateBlock>
        </Panel>
      </div>

      <div className="content-grid">
        <Panel title="Platform status">
          <StateBlock
            loading={platformStatus.loading}
            error={platformStatus.error}
            empty={!platformStatus.data ? "No platform status available." : null}
          >
            <div
              className={`status-banner ${
                platformStatus.data?.overall === "fail"
                  ? "status-banner--fail"
                  : platformStatus.data?.overall === "warning"
                    ? "status-banner--warning"
                    : "status-banner--ok"
              }`}
            >
              <strong>Overall {platformStatus.data?.overall.toUpperCase()}</strong>
              <span className="muted">
                {platformStatus.data?.checks.filter((item) => item.level !== "ok").length ?? 0} issues
              </span>
            </div>
            <div className="detail-stack">
              {platformStatus.data?.checks.map((check) => (
                <div key={check.key} className={`status-card status-card--${check.level}`}>
                  <div className="status-card__head">
                    <span className={`status-pill status-pill--${check.level}`}>
                      {check.level.toUpperCase()}
                    </span>
                    <strong>{check.label}</strong>
                  </div>
                  <p>{check.message}</p>
                </div>
              ))}
            </div>
          </StateBlock>
        </Panel>

        <Panel title="Roadmap">
          <StateBlock
            loading={roadmapItems.loading}
            error={roadmapItems.error}
            empty={roadmapItems.data?.length === 0 ? "Nothing on the roadmap yet." : null}
          >
            <ul className="plain-list">
              {roadmapItems.data?.slice(0, 8).map((item) => (
                <li key={item.id}>
                  <span>{item.title}</span>
                  <span className="badge">{humanizeKey(item.status)}</span>
                </li>
              ))}
            </ul>
          </StateBlock>
        </Panel>

        <Panel title="Recent activity">
          <StateBlock
            loading={audit.loading}
            error={audit.error}
            empty={audit.data?.items.length === 0 ? "No activity yet." : null}
          >
            <ul className="timeline-list">
              {audit.data?.items.map((item) => (
                <li key={item.id}>
                  <div>
                    <strong>{humanizeKey(item.action)}</strong>
                    <div className="muted">
                      {item.actor_display_name} - {item.entity_type}
                    </div>
                  </div>
                  <time>{formatDateTime(item.created_at)}</time>
                </li>
              ))}
            </ul>
          </StateBlock>
        </Panel>
      </div>
    </section>
  );
}
