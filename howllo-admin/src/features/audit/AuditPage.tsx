import { useState } from "react";
import { admin } from "@howllo/api-client";
import type { AuditLogItem, PaginatedResponse } from "@howllo/types";
import { Panel } from "../../components/Panel";
import { SessionRequired } from "../../components/SessionRequired";
import { StateBlock } from "../../components/StateBlock";
import IdChip from "../../components/IdChip";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";
import { formatDateTime, humanizeKey } from "../../lib/format";

const PER_PAGE = 20;

export function AuditPage() {
  const { client, tenant, authorization } = useSession();
  const api = admin(client);
  const [page, setPage] = useState(1);
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const logs = useAsync<PaginatedResponse<AuditLogItem>>(
    () =>
      authorization
        ? api.listAuditLogs(page, PER_PAGE)
        : Promise.resolve({
            items: [],
            page: 1,
            per_page: PER_PAGE,
            total: 0,
            has_next: false,
          }),
    [tenant, authorization, page],
  );
  const searchResults = useAsync<AuditLogItem[]>(
    async () => {
      if (!authorization || !needle) return [];

      const matches = (item: AuditLogItem) =>
        item.action.toLowerCase().includes(needle) ||
        item.actor_display_name.toLowerCase().includes(needle) ||
        item.entity_type.toLowerCase().includes(needle) ||
        item.entity_id.toLowerCase().includes(needle) ||
        (item.request_id ?? "").toLowerCase().includes(needle);

      const items: AuditLogItem[] = [];
      let nextPage = 1;

      while (true) {
        const response = await api.listAuditLogs(nextPage, PER_PAGE);
        items.push(...response.items.filter(matches));
        if (!response.has_next) break;
        nextPage += 1;
      }

      return items;
    },
    [tenant, authorization, needle],
  );

  if (!authorization) {
    return (
      <section>
        <h1>Audit log</h1>
        <SessionRequired />
      </section>
    );
  }

  const filtered = needle ? searchResults.data ?? [] : logs.data?.items ?? [];
  const isSearching = Boolean(needle);
  const isLoading = isSearching ? searchResults.loading : logs.loading;
  const error = isSearching ? searchResults.error : logs.error;

  return (
    <section>
      <h1>Audit log</h1>
      <p className="muted">
        Review high-privilege workspace actions with actor and request trace data.
      </p>

      <Panel
        title="Recent actions"
        actions={
          <div className="audit-toolbar">
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Filter action, actor, entity, request"
            />
            <button
              disabled={isSearching || page === 1}
              onClick={() => setPage((value) => value - 1)}
            >
              Previous
            </button>
            <button
              disabled={isSearching || !logs.data?.has_next}
              onClick={() => setPage((value) => value + 1)}
            >
              Next
            </button>
          </div>
        }
      >
        {isSearching ? (
          <p className="muted small" style={{ marginTop: 0 }}>
            Searching across all loaded audit pages. Pagination is disabled while a filter is active.
          </p>
        ) : null}
        <StateBlock
          loading={isLoading}
          error={error}
          empty={filtered.length === 0 ? "No audit entries found." : null}
        >
          <table className="data-table">
            <thead>
              <tr>
                <th>Action</th>
                <th>Actor</th>
                <th>Entity</th>
                <th>Request</th>
                <th>When</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((item) => (
                <tr key={item.id}>
                  <td className="cell-nowrap">
                    <strong>{humanizeKey(item.action)}</strong>
                  </td>
                  <td className="cell-nowrap">{item.actor_display_name}</td>
                  <td className="cell-nowrap">
                    <div className="audit-inline-cell">
                      <span className="audit-inline-label">{item.entity_type}</span>
                      <IdChip value={item.entity_id} />
                    </div>
                  </td>
                  <td className="cell-nowrap">
                    <div className="audit-inline-cell">
                      <IdChip value={item.request_id} />
                      {item.old_value || item.new_value ? (
                        <details className="audit-diff">
                          <summary>View diff</summary>
                          {item.old_value ? (
                            <pre className="response-preview">
                              old: {JSON.stringify(item.old_value, null, 2)}
                            </pre>
                          ) : null}
                          {item.new_value ? (
                            <pre className="response-preview">
                              new: {JSON.stringify(item.new_value, null, 2)}
                            </pre>
                          ) : null}
                        </details>
                      ) : null}
                    </div>
                  </td>
                  <td className="cell-nowrap muted">{formatDateTime(item.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </StateBlock>
      </Panel>
    </section>
  );
}
