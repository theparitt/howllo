import { useState } from "react";
import { admin } from "@howllo/api-client";
import type {
  ApiTokenCreated,
  ApiTokenListItem,
  WebhookDelivery,
  WebhookEndpoint,
} from "@howllo/types";
import { Panel } from "../../components/Panel";
import { SessionRequired } from "../../components/SessionRequired";
import { StateBlock } from "../../components/StateBlock";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";
import { formatDateTime } from "../../lib/format";

const DEFAULT_SCOPES = "posts:read";

export function IntegrationsPage() {
  const { client, tenant, authorization } = useSession();
  const api = admin(client);
  const webhooks = useAsync<WebhookEndpoint[]>(
    () => (authorization ? api.listWebhooks() : Promise.resolve([])),
    [tenant, authorization],
  );
  const deliveries = useAsync<WebhookDelivery[]>(
    () => (authorization ? api.listWebhookDeliveries() : Promise.resolve([])),
    [tenant, authorization],
  );
  const tokens = useAsync<ApiTokenListItem[]>(
    () => (authorization ? api.listApiTokens() : Promise.resolve([])),
    [tenant, authorization],
  );

  if (!authorization) {
    return (
      <section>
        <h1>Integrations</h1>
        <SessionRequired />
      </section>
    );
  }

  return (
    <section>
      <h1>Integrations</h1>
      <p className="muted">
        Manage workspace webhooks, delivery retries, and scoped API tokens.
      </p>

      <CreateWebhook onCreated={() => {
        webhooks.reload();
        deliveries.reload();
      }} />

      <div className="content-grid">
        <Panel title="Webhook endpoints">
          <StateBlock
            loading={webhooks.loading}
            error={webhooks.error}
            empty={webhooks.data?.length === 0 ? "No webhook endpoints yet." : null}
          >
            <table className="data-table">
              <thead>
                <tr>
                  <th>URL</th>
                  <th>Secret</th>
                  <th>Status</th>
                  <th>Created</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {webhooks.data?.map((webhook) => (
                  <WebhookRow
                    key={webhook.id}
                    webhook={webhook}
                    onChanged={() => {
                      webhooks.reload();
                      deliveries.reload();
                    }}
                  />
                ))}
              </tbody>
            </table>
          </StateBlock>
        </Panel>

        <Panel title="API tokens">
          <CreateToken onCreated={tokens.reload} />
          <StateBlock
            loading={tokens.loading}
            error={tokens.error}
            empty={tokens.data?.length === 0 ? "No API tokens issued." : null}
          >
            <table className="data-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Prefix</th>
                  <th>Scopes</th>
                  <th>Status</th>
                  <th>Created</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {tokens.data?.map((token) => (
                  <TokenRow key={token.id} token={token} onChanged={tokens.reload} />
                ))}
              </tbody>
            </table>
          </StateBlock>
        </Panel>
      </div>

      <Panel title="Recent deliveries">
        <StateBlock
          loading={deliveries.loading}
          error={deliveries.error}
          empty={deliveries.data?.length === 0 ? "No delivery attempts recorded." : null}
        >
          <table className="data-table">
            <thead>
                <tr>
                  <th>Delivery</th>
                  <th>Status</th>
                  <th>Attempts</th>
                  <th>Retry</th>
                  <th>When</th>
                  <th>Response</th>
                  <th />
                </tr>
            </thead>
            <tbody>
              {deliveries.data?.map((delivery) => (
                <DeliveryRow
                  key={delivery.id}
                  delivery={delivery}
                  onChanged={deliveries.reload}
                />
              ))}
            </tbody>
          </table>
        </StateBlock>
      </Panel>
    </section>
  );
}

function CreateWebhook({ onCreated }: { onCreated: () => void }) {
  const { client } = useSession();
  const api = admin(client);
  const [url, setUrl] = useState("-");
  const [secret, setSecret] = useState("-");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.createWebhook({
        url: url.trim(),
        secret: secret.trim() || undefined,
      });
      setUrl("-");
      setSecret("-");
      onCreated();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create webhook");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel title="Add webhook">
      <div className="form-row">
        <label className="field-grow">
          Endpoint URL
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://example.com/howllo/webhook"
          />
        </label>
        <label className="field-grow">
          Secret
          <input
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            placeholder="Optional signing secret"
          />
        </label>
        <button className="primary" disabled={busy || !url.trim()} onClick={submit}>
          {busy ? "Creating..." : "Create webhook"}
        </button>
      </div>
      {error ? <p className="error-text">{error}</p> : null}
    </Panel>
  );
}

function WebhookRow({
  webhook,
  onChanged,
}: {
  webhook: WebhookEndpoint;
  onChanged: () => void;
}) {
  const { client } = useSession();
  const api = admin(client);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const deactivate = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.deactivateWebhook(webhook.id);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to deactivate webhook");
    } finally {
      setBusy(false);
    }
  };

  return (
    <tr>
      <td>
        <code>{webhook.url}</code>
        {error ? <div className="error-text">{error}</div> : null}
      </td>
      <td>{webhook.has_secret ? "Configured" : "None"}</td>
      <td>{webhook.is_active ? "Active" : "Inactive"}</td>
      <td>{formatDateTime(webhook.created_at)}</td>
      <td className="row-actions">
        {webhook.is_active ? (
          <button className="danger small" disabled={busy} onClick={deactivate}>
            Deactivate
          </button>
        ) : null}
      </td>
    </tr>
  );
}

function CreateToken({ onCreated }: { onCreated: () => void }) {
  const { client } = useSession();
  const api = admin(client);
  const [name, setName] = useState("");
  const [scopes, setScopes] = useState(DEFAULT_SCOPES);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<ApiTokenCreated | null>(null);
  const [copyMessage, setCopyMessage] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    setCreated(null);
    setCopyMessage(null);
    try {
      const token = await api.createApiToken({
        name: name.trim(),
        scopes: scopes
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean),
      });
      setCreated(token);
      setName("");
      onCreated();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create API token");
    } finally {
      setBusy(false);
    }
  };

  const copyToken = async () => {
    if (!created) return;
    setCopyMessage(null);
    try {
      await navigator.clipboard.writeText(created.token);
      setCopyMessage("Token copied.");
    } catch (e) {
      setCopyMessage(e instanceof Error ? e.message : "Copy failed.");
    }
  };

  return (
    <div className="detail-stack">
      <div className="form-row">
        <label>
          Name
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Post sync token"
          />
        </label>
        <label className="field-grow">
          Scopes
          <input
            value={scopes}
            onChange={(e) => setScopes(e.target.value)}
            placeholder="posts:read"
          />
        </label>
        <button className="primary" disabled={busy || !name.trim()} onClick={submit}>
          {busy ? "Creating..." : "Create token"}
        </button>
      </div>
      {created ? (
        <div className="callout">
          <strong>Copy this token now.</strong>
          <code>{created.token}</code>
          <div className="button-row" style={{ marginTop: 8 }}>
            <button onClick={() => void copyToken()}>
              Copy token
            </button>
          </div>
          {copyMessage ? <div className="muted">{copyMessage}</div> : null}
        </div>
      ) : null}
      {error ? <p className="error-text">{error}</p> : null}
    </div>
  );
}

function TokenRow({
  token,
  onChanged,
}: {
  token: ApiTokenListItem;
  onChanged: () => void;
}) {
  const { client } = useSession();
  const api = admin(client);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const revoke = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.revokeApiToken(token.id);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to revoke token");
    } finally {
      setBusy(false);
    }
  };

  return (
    <tr>
      <td>
        <strong>{token.name}</strong>
        {error ? <div className="error-text">{error}</div> : null}
      </td>
      <td>
        <code>{token.token_prefix}</code>
      </td>
      <td>{token.scopes.join("-")}</td>
      <td>{token.revoked_at ? "Revoked" : "Active"}</td>
      <td>{formatDateTime(token.created_at)}</td>
      <td className="row-actions">
        {!token.revoked_at ? (
          <button className="danger small" disabled={busy} onClick={revoke}>
            Revoke
          </button>
        ) : null}
      </td>
    </tr>
  );
}

function DeliveryRow({
  delivery,
  onChanged,
}: {
  delivery: WebhookDelivery;
  onChanged: () => void;
}) {
  const { client } = useSession();
  const api = admin(client);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const resend = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.resendWebhookDelivery(delivery.id);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to resend delivery");
    } finally {
      setBusy(false);
    }
  };

  return (
    <tr>
      <td>
        <code>{delivery.id.slice(0, 8)}</code>
        {error ? <div className="error-text">{error}</div> : null}
      </td>
      <td>
        <span className={delivery.success ? "status-dot success" : "status-dot danger"}>
          {delivery.success ? "Success" : "Failed"}
        </span>
        {delivery.status_code ? <div className="muted">{delivery.status_code}</div> : null}
      </td>
      <td>{delivery.attempt_count}</td>
      <td>{formatDateTime(delivery.next_retry_at)}</td>
      <td>{formatDateTime(delivery.delivered_at)}</td>
      <td>
        {delivery.response_body ? (
          <details>
            <summary>View</summary>
            <pre className="response-preview">{delivery.response_body}</pre>
          </details>
        ) : ("-")}
      </td>
      <td className="row-actions">
        <button disabled={busy} onClick={resend}>
          Resend
        </button>
      </td>
    </tr>
  );
}
