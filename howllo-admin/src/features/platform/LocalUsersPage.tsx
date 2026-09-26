import { useEffect, useState, type FormEvent } from "react";
import { API_BASE_URL } from "@howllo/config";
import { Panel } from "../../components/Panel";
import { useSession } from "../../lib/session";

type LocalUser = {
  user_id: string;
  username: string;
  display_name: string;
  created_at: string;
  last_sign_in_at: string | null;
  login_ip: string | null;
  user_agent: string | null;
};

type Page = { items: LocalUser[]; page: number; per_page: number; total: number; has_next: boolean };

function device(userAgent: string | null): string {
  if (!userAgent) return "Unknown device";
  const browser = userAgent.match(/Edg\/([\d.]+)/)?.[1];
  const chrome = userAgent.match(/Chrome\/([\d.]+)/)?.[1];
  const firefox = userAgent.match(/Firefox\/([\d.]+)/)?.[1];
  const safari = userAgent.match(/Version\/([\d.]+).*Safari\//)?.[1];
  const browserName = browser ? `Edge ${browser}` : chrome ? `Chrome ${chrome}` : firefox ? `Firefox ${firefox}` : safari ? `Safari ${safari}` : "Other browser";
  const os = /Android ([\d.]+)/.exec(userAgent)?.[1];
  const ios = /(?:iPhone|iPad).*OS ([\d_]+)/.exec(userAgent)?.[1];
  const mac = /Mac OS X ([\d_.]+)/.exec(userAgent)?.[1];
  const system = os ? `Android ${os}` : ios ? `iOS ${ios.replaceAll("_", ".")}` : /Windows NT/.test(userAgent) ? "Windows" : mac ? `macOS ${mac.replaceAll("_", ".")}` : /Linux/.test(userAgent) ? "Linux" : "Unknown OS";
  return `${system} · ${browserName}`;
}

function date(value: string | null): string {
  return value ? new Date(value).toLocaleString() : "No recorded sign-in";
}

async function readJson<T>(response: Response): Promise<T> {
  const body = await response.json().catch(() => ({})) as { error?: { message?: string } };
  if (!response.ok) throw new Error(body.error?.message || `Request failed (${response.status})`);
  return body as T;
}

export function LocalUsersPage() {
  const { authorization } = useSession();
  const [search, setSearch] = useState("");
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<Page | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<LocalUser | null>(null);
  const [password, setPassword] = useState("");
  const [reason, setReason] = useState("");
  const [code, setCode] = useState("");
  const [issuing, setIssuing] = useState(false);

  useEffect(() => {
    const timeout = window.setTimeout(() => { setQuery(search.trim()); setPage(1); }, 250);
    return () => window.clearTimeout(timeout);
  }, [search]);

  useEffect(() => {
    if (!authorization) return;
    const controller = new AbortController();
    const params = new URLSearchParams({ search: query, page: String(page), per_page: "20" });
    setLoading(true);
    setError("");
    fetch(`${API_BASE_URL}/api/admin/local-users?${params}`, {
      headers: { Authorization: authorization }, signal: controller.signal, cache: "no-store",
    }).then(readJson<Page>).then(setData).catch((cause: unknown) => {
      if (!controller.signal.aborted) setError(cause instanceof Error ? cause.message : "Could not load accounts.");
    }).finally(() => { if (!controller.signal.aborted) setLoading(false); });
    return () => controller.abort();
  }, [authorization, page, query]);

  function choose(user: LocalUser) {
    setSelected(user);
    setPassword("");
    setReason("");
    setCode("");
    setError("");
  }

  async function issue(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selected || !authorization) return;
    setIssuing(true);
    setError("");
    setCode("");
    try {
      const result = await readJson<{ recovery_code: string }>(await fetch(`${API_BASE_URL}/api/admin/local-users/${encodeURIComponent(selected.user_id)}/recovery`, {
        method: "POST",
        headers: { Authorization: authorization, "Content-Type": "application/json" },
        cache: "no-store",
        body: JSON.stringify({ admin_password: password, reason: reason.trim() }),
      }));
      setCode(result.recovery_code);
      setPassword("");
      setReason("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not issue a recovery code.");
    } finally { setIssuing(false); }
  }

  return <section className="local-users-page">
    <h1>User accounts</h1>
    <p className="muted">Local sign-in accounts across all workspaces. Staff and workspace membership are managed separately.</p>
    <Panel title="Find an account">
      <label className="local-users-search">Search by username or display name
        <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search accounts" maxLength={100} />
      </label>
      {loading ? <p className="muted">Loading…</p> : null}
      {error ? <p role="alert" className="error-text">{error}</p> : null}
      {data && !loading ? <>
        <div className="local-users-table-wrap"><table className="data-table">
          <thead><tr><th>Account</th><th>Last sign-in</th><th>IP address</th><th>Device</th><th></th></tr></thead>
          <tbody>{data.items.map((user) => <tr key={user.user_id}>
            <td><strong>{user.display_name}</strong><br /><span className="muted">{user.username}</span></td>
            <td>{date(user.last_sign_in_at)}</td>
            <td><code>{user.login_ip || "—"}</code></td>
            <td title={user.user_agent || undefined}>{device(user.user_agent)}</td>
            <td><button type="button" onClick={() => choose(user)}>Recovery</button></td>
          </tr>)}</tbody>
        </table></div>
        {data.items.length === 0 ? <p className="muted">No accounts found.</p> : null}
        <div className="local-users-pager"><span>{data.total} accounts · Page {data.page} of {Math.max(1, Math.ceil(data.total / data.per_page))}</span>
          <div><button type="button" disabled={page <= 1} onClick={() => setPage(page - 1)}>Previous</button>
            <button type="button" disabled={!data.has_next} onClick={() => setPage(page + 1)}>Next</button></div>
        </div>
      </> : null}
    </Panel>
    {selected ? <Panel title={`Recover ${selected.username}`}>
      {code ? <div className="local-users-recovery" role="status">
        <p>Share this code with the account owner through a trusted channel. It is shown only here and replaces the previous code.</p>
        <code>{code}</code>
        <button type="button" onClick={() => void navigator.clipboard.writeText(code)}>Copy code</button>
        <p>They can use “Forgot password? Use recovery code” on the board sign-in page. Existing sessions were signed out.</p>
        <button type="button" onClick={() => { setSelected(null); setCode(""); }}>Done</button>
      </div> : <form className="local-users-recovery" onSubmit={(event) => void issue(event)}>
        <p className="muted">This replaces the existing recovery code and signs this account out everywhere.</p>
        <label>Reason for recovery<input value={reason} onChange={(event) => setReason(event.target.value)} minLength={8} maxLength={500} required placeholder="Support request reference" /></label>
        <label>Your admin password<input type="password" autoComplete="current-password" value={password} onChange={(event) => setPassword(event.target.value)} required /></label>
        <div className="row-actions"><button type="submit" disabled={issuing}>{issuing ? "Issuing…" : "Issue one-time code"}</button><button type="button" onClick={() => setSelected(null)}>Cancel</button></div>
      </form>}
    </Panel> : null}
  </section>;
}
