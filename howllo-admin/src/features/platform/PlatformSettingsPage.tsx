import { useEffect, useState } from "react";
import { admin } from "@howllo/api-client";
import type {
  PlatformDatabaseInfo,
  PlatformStorageConfig,
  PlatformStatusResponse,
  StorageBackend,
  WorkspaceStorageUsageResponse,
} from "@howllo/types";
import { useSession } from "../../lib/session";
import { Panel } from "../../components/Panel";
import { PlatformPolicyTab } from "./PlatformPolicyTab";

type Tab = "status" | "storage" | "database" | "limits";

export function PlatformSettingsPage() {
  const [tab, setTab] = useState<Tab>("status");

  return (
    <section>
      <h1>Platform settings</h1>
      <p className="muted">
        Server-wide configuration. Values default from environment variables and
        can be overridden here after a successful test.
      </p>

      <div className="seg-tabs">
        <button
          className={tab === "status" ? "seg-tab active" : "seg-tab"}
          onClick={() => setTab("status")}
        >
          Status
        </button>
        <button
          className={tab === "storage" ? "seg-tab active" : "seg-tab"}
          onClick={() => setTab("storage")}
        >
          Storage
        </button>
        <button
          className={tab === "database" ? "seg-tab active" : "seg-tab"}
          onClick={() => setTab("database")}
        >
          Database
        </button>
        <button className={tab === "limits" ? "seg-tab active" : "seg-tab"} onClick={() => setTab("limits")}>Limits</button>
      </div>

      {tab === "status" ? (
        <StatusTab />
      ) : tab === "storage" ? (
        <StorageTab />
      ) : tab === "limits" ? (
        <PlatformPolicyTab />
      ) : (
        <DatabaseTab />
      )}
    </section>
  );
}

// A 401/403 means the admin session is missing/expired/not the local admin.
// Surface a clear, actionable message instead of a bare "Unauthorized".
function authMessage(e: unknown): string {
  const status = (e as { status?: number })?.status;
  if (status === 401 || status === 403) {
    return "Your admin session has expired or is not signed in. Log out and sign in again, then reopen this page.";
  }
  return e instanceof Error ? e.message : "Request failed.";
}

function StorageTab() {
  const { client, logout } = useSession();
  const api = admin(client);

  const [config, setConfig] = useState<PlatformStorageConfig | null>(null);
  const [backend, setBackend] = useState<StorageBackend>("local");
  const [localPath, setLocalPath] = useState("");
  const [publicBaseUrl, setPublicBaseUrl] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [bucket, setBucket] = useState("");
  const [accessKey, setAccessKey] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [useSsl, setUseSsl] = useState(true);

  const [loading, setLoading] = useState(true);
  const [usageLoading, setUsageLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [ok, setOk] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [usageError, setUsageError] = useState<string | null>(null);
  const [authError, setAuthError] = useState(false);
  const [usage, setUsage] = useState<WorkspaceStorageUsageResponse | null>(null);

  useEffect(() => {
    api
      .getStorageConfig()
      .then((cfg) => {
        setConfig(cfg);
        setBackend(cfg.backend);
        setLocalPath(cfg.local_path);
        setPublicBaseUrl(cfg.public_base_url);
        setEndpoint(cfg.minio_endpoint);
        setBucket(cfg.minio_bucket);
        setAccessKey(cfg.minio_access_key);
        setUseSsl(cfg.minio_use_ssl);
      })
      .catch((e) => {
        const status = (e as { status?: number })?.status;
        if (status === 401 || status === 403) setAuthError(true);
        setError(authMessage(e));
      })
      .finally(() => setLoading(false));

    api
      .getWorkspaceStorageUsage()
      .then((data) => setUsage(data))
      .catch((e) => {
        const status = (e as { status?: number })?.status;
        if (status === 401 || status === 403) setAuthError(true);
        setUsageError(authMessage(e));
      })
      .finally(() => setUsageLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const reset = () => {
    setOk(null);
    setError(null);
  };

  const reloadUsage = async () => {
    setUsageLoading(true);
    setUsageError(null);
    try {
      setUsage(await api.getWorkspaceStorageUsage());
    } catch (e) {
      const status = (e as { status?: number })?.status;
      if (status === 401 || status === 403) setAuthError(true);
      setUsageError(authMessage(e));
    } finally {
      setUsageLoading(false);
    }
  };

  // Test must pass before save - on success, we save automatically.
  const testAndSave = async () => {
    setBusy(true);
    reset();
    try {
      const test = await api.testStorage({
        backend,
        local_path: localPath,
        public_base_url: publicBaseUrl,
        minio_endpoint: endpoint,
        minio_bucket: bucket,
        minio_access_key: accessKey,
        minio_secret_key: secretKey || undefined,
        minio_use_ssl: useSsl,
      });
      const saved = await api.saveStorageConfig({
        backend,
        local_path: localPath,
        public_base_url: publicBaseUrl,
        minio_endpoint: endpoint,
        minio_bucket: bucket,
        minio_access_key: accessKey,
        minio_secret_key: secretKey || undefined,
        minio_use_ssl: useSsl,
      });
      setConfig(saved);
      setSecretKey("");
      setOk(`${test.message} Saved.`);
    } catch (e) {
      const status = (e as { status?: number })?.status;
      if (status === 401 || status === 403) setAuthError(true);
      setError(authMessage(e));
    } finally {
      setBusy(false);
    }
  };

  if (loading) return <p className="muted">Loading storage settings...</p>;

  if (authError) {
    return (
      <Panel title="Session expired">
        <p className="muted" style={{ marginBottom: 16 }}>
          {error ??
            "Your admin session has expired. Sign in again to manage storage."}
        </p>
        <button className="primary" onClick={logout}>
          Sign in again
        </button>
      </Panel>
    );
  }

  const activeBackend = config?.backend ?? null;
  const activeLabel =
    activeBackend === "minio" ? "MinIO" : activeBackend === "local" ? "Local disk" : "-";
  const sourceLabel = config?.backend_configured ? "saved in database" : "from environment default";
  const editingDiffersFromActive = activeBackend !== null && backend !== activeBackend;

  return (
    <Panel title="File storage">
      <p className="muted" style={{ marginBottom: 14 }}>
        Where uploaded images (post screenshots, etc.) are stored.
      </p>

      {/* Unambiguous banner of what is ACTIVE right now. */}
      <div className="active-backend">
        <span className="active-backend__dot" />
        <span>
          Active storage: <strong>{activeLabel}</strong>
          <span className="active-backend__source"> - {sourceLabel}</span>
        </span>
      </div>

      <div className="storage-mode-label">Select a backend to view or edit</div>
      <div className="seg-tabs" style={{ marginBottom: 16 }}>
        {(["local", "minio"] as StorageBackend[]).map((b) => (
          <button
            key={b}
            className={backend === b ? "seg-tab active" : "seg-tab"}
            onClick={() => {
              setBackend(b);
              reset();
            }}
          >
            {b === "local" ? "Local disk" : "MinIO"}
            {activeBackend === b ? <span className="seg-tab__badge">ACTIVE</span> : null}
          </button>
        ))}
      </div>

      {editingDiffersFromActive ? (
        <div className="storage-warn">
          You&rsquo;re editing <strong>{backend === "minio" ? "MinIO" : "Local disk"}</strong>,
          which is <strong>not</strong> the active backend. Test &amp; save to switch to it.
        </div>
      ) : null}

      {backend === "local" ? (
        <div className="form-grid">
          <label className="field-span-2">
            Storage directory
            <input
              value={localPath}
              onChange={(e) => {
                setLocalPath(e.target.value);
                reset();
              }}
              placeholder="./data/uploads"
            />
          </label>
          <label className="field-span-2">
            Public base URL
            <input
              value={publicBaseUrl}
              onChange={(e) => {
                setPublicBaseUrl(e.target.value);
                reset();
              }}
              placeholder="http://127.0.0.1:7700/uploads"
            />
          </label>
        </div>
      ) : (
        <div className="form-grid">
          <label className="field-span-2">
            Endpoint
            <input
              value={endpoint}
              onChange={(e) => {
                setEndpoint(e.target.value);
                const v = e.target.value.trim().toLowerCase();
                if (v.startsWith("https://")) setUseSsl(true);
                else if (v.startsWith("http://")) setUseSsl(false);
                reset();
              }}
              placeholder="http://127.0.0.1:9000"
            />
          </label>
          <label>
            Bucket
            <input
              value={bucket}
              onChange={(e) => {
                setBucket(e.target.value);
                reset();
              }}
              placeholder="howllo"
            />
          </label>
          <label>
            Access key
            <input
              value={accessKey}
              onChange={(e) => {
                setAccessKey(e.target.value);
                reset();
              }}
              placeholder="minioadmin"
            />
          </label>
          <label>
            Secret key
            <input
              type="password"
              value={secretKey}
              onChange={(e) => {
                setSecretKey(e.target.value);
                reset();
              }}
              placeholder={
                config?.minio_secret_key_configured
                  ? "Stored - leave blank to keep"
                  : "minioadmin"
              }
            />
          </label>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={useSsl}
              onChange={(e) => {
                setUseSsl(e.target.checked);
                reset();
              }}
            />
            Use HTTPS / SSL
          </label>
          <label className="field-span-2">
            Public base URL
            <input
              value={publicBaseUrl}
              onChange={(e) => {
                setPublicBaseUrl(e.target.value);
                reset();
              }}
              placeholder="http://127.0.0.1:9000/howllo"
            />
          </label>
        </div>
      )}

      {ok ? <p className="storage-ok">{ok}</p> : null}
      {error ? <p className="error-text">{error}</p> : null}

      <div className="button-row" style={{ marginTop: 16 }}>
        <button className="primary" disabled={busy} onClick={testAndSave}>
          {busy ? "Testing..." : "Test & save"}
        </button>
        <span className="muted small">
          {backend === "minio"
            ? "Tests a real upload -> anonymous read -> delete round-trip."
            : "Verifies the directory is writable."}
        </span>
      </div>

      <div style={{ height: 18 }} />

      <Panel
        title="Workspace storage usage"
        actions={
          <button disabled={usageLoading} onClick={() => void reloadUsage()}>
            {usageLoading ? "Refreshing..." : "Refresh"}
          </button>
        }
      >
        <p className="muted" style={{ marginBottom: 16 }}>
          Real file usage by workspace, based on stored logos and post attachment
          assets in the active backend.
        </p>

        {usage ? (
          <div className="detail-list" style={{ marginBottom: 16 }}>
            <div>
              <dt>Backend</dt>
              <dd>{usage.backend === "minio" ? "MinIO" : "Local disk"}</dd>
            </div>
            <div>
              <dt>Total assets</dt>
              <dd>{usage.total_assets}</dd>
            </div>
            <div>
              <dt>Total size</dt>
              <dd>{formatBytes(usage.total_bytes)}</dd>
            </div>
          </div>
        ) : null}

        {usageError ? <p className="error-text">{usageError}</p> : null}

        {!usageLoading && usage && usage.items.length === 0 ? (
          <p className="muted">No stored workspace assets found yet.</p>
        ) : null}

        {usage ? (
          <table className="data-table">
            <thead>
              <tr>
                <th>Workspace</th>
                <th>Assets</th>
                <th>Logos</th>
                <th>Attachments</th>
                <th>Total</th>
              </tr>
            </thead>
            <tbody>
              {usage.items.map((item) => (
                <tr key={item.workspace_id}>
                  <td>
                    <strong>{item.workspace_name}</strong>
                    <div className="muted">
                      <code>{item.workspace_slug}</code>
                    </div>
                  </td>
                  <td>{item.asset_count}</td>
                  <td>{formatBytes(item.logo_bytes)}</td>
                  <td>{formatBytes(item.attachment_bytes)}</td>
                  <td>{formatBytes(item.total_bytes)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : null}
      </Panel>
    </Panel>
  );
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = value / 1024;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[index]}`;
}

function StatusTab() {
  const { client, logout } = useSession();
  const api = admin(client);
  const [status, setStatus] = useState<PlatformStatusResponse | null>(null);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [authError, setAuthError] = useState(false);

  const load = async () => {
    setBusy(true);
    setError(null);
    try {
      setStatus(await api.getPlatformStatus());
    } catch (e) {
      const status = (e as { status?: number })?.status;
      if (status === 401 || status === 403) setAuthError(true);
      setError(authMessage(e));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (authError) {
    return (
      <Panel title="Session expired">
        <p className="muted" style={{ marginBottom: 16 }}>
          {error ??
            "Your admin session has expired. Sign in again to inspect platform status."}
        </p>
        <button className="primary" onClick={logout}>
          Sign in again
        </button>
      </Panel>
    );
  }

  return (
    <Panel
      title="System status"
      actions={
        <button disabled={busy} onClick={() => void load()}>
          {busy ? "Checking..." : "Refresh"}
        </button>
      }
    >
      <p className="muted" style={{ marginBottom: 16 }}>
        Live health checks for core platform dependencies. Failures here should
        explain why the system is unhealthy.
      </p>

      {status ? (
        <div
          className={`status-banner ${
            status.overall === "fail"
              ? "status-banner--fail"
              : status.overall === "warning"
                ? "status-banner--warning"
                : "status-banner--ok"
          }`}
        >
          <strong>
            Overall:{" "}
            {status.overall === "fail"
              ? "Failure"
              : status.overall === "warning"
                ? "Warning"
                : "OK"}
          </strong>
          <span className="muted">
            Checked at {new Date(status.checked_at).toLocaleString()}
          </span>
        </div>
      ) : null}

      {error ? <p className="error-text">{error}</p> : null}

      {status ? (
        <div className="status-grid">
          {status.checks.map((check) => (
            <div
              key={check.key}
              className={`status-card status-card--${check.level}`}
            >
              <div className="status-card__head">
                <span className={`status-pill status-pill--${check.level}`}>
                  {check.level === "fail"
                    ? "FAIL"
                    : check.level === "warning"
                      ? "WARN"
                      : "OK"}
                </span>
                <strong>{check.label}</strong>
              </div>
              <p>{check.message}</p>
              {check.detail ? <p className="muted small">{check.detail}</p> : null}
            </div>
          ))}
        </div>
      ) : null}
    </Panel>
  );
}

function DatabaseTab() {
  const { client } = useSession();
  const api = admin(client);
  const [info, setInfo] = useState<PlatformDatabaseInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const check = async () => {
    setBusy(true);
    setError(null);
    try {
      setInfo(await api.testDatabase());
    } catch (e) {
      setError(e instanceof Error ? e.message : "Database check failed.");
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void check();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <Panel title="Database">
      <p className="muted" style={{ marginBottom: 16 }}>
        PostgreSQL connection. Read-only - the database URL is configured via{" "}
        <code>HOWLLO_DATABASE_URL</code> before startup.
      </p>

      {info ? (
        <div className="detail-list">
          <div>
            <dt>Connection</dt>
            <dd className="mono">{info.url_masked}</dd>
          </div>
          <div>
            <dt>Host</dt>
            <dd>
              {info.host}:{info.port}
            </dd>
          </div>
          <div>
            <dt>Database</dt>
            <dd>{info.database}</dd>
          </div>
          <div>
            <dt>Migrations applied</dt>
            <dd>{info.migration_count}</dd>
          </div>
        </div>
      ) : null}

      {info ? <p className="storage-ok">{info.message}</p> : null}
      {error ? <p className="error-text">{error}</p> : null}

      <div className="button-row" style={{ marginTop: 16 }}>
        <button className="primary" disabled={busy} onClick={check}>
          {busy ? "Checking..." : "Run connection check"}
        </button>
      </div>
    </Panel>
  );
}
