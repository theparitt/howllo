import { useEffect, useState } from "react";
import { admin } from "@howllo/api-client";
import type { MembershipItem } from "@howllo/types";
import { Panel } from "../../components/Panel";
import { SessionRequired } from "../../components/SessionRequired";
import { StateBlock } from "../../components/StateBlock";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";

const ROLES = ["owner", "admin", "moderator", "member"];

export function MembersPage() {
  const { client, tenant, authorization } = useSession();
  const api = admin(client);
  const members = useAsync<MembershipItem[]>(
    () => (authorization ? api.listMembers() : Promise.resolve([])),
    [tenant, authorization],
  );

  if (!authorization) {
    return (
      <section>
        <h1>Members</h1>
        <SessionRequired />
      </section>
    );
  }

  return (
    <section>
      <h1>Members</h1>
      <p className="muted">Manage workspace access and role boundaries.</p>

      <CreateMember onCreated={members.reload} />

      <Panel title="Current members">
        <StateBlock
          loading={members.loading}
          error={members.error}
          empty={members.data?.length === 0 ? "No members found." : null}
        >
          <table className="data-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Email</th>
                <th>Role</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {members.data?.map((member) => (
                <MemberRow
                  key={member.user_id}
                  member={member}
                  ownerCount={(members.data ?? []).filter((item) => item.role === "owner").length}
                  onChanged={members.reload}
                />
              ))}
            </tbody>
          </table>
        </StateBlock>
      </Panel>
    </section>
  );
}

function CreateMember({ onCreated }: { onCreated: () => void }) {
  const { client } = useSession();
  const api = admin(client);
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [role, setRole] = useState("member");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.createMember({
        email: email.trim(),
        display_name: displayName.trim() || undefined,
        role,
      });
      setEmail("");
      setDisplayName("");
      setRole("member");
      onCreated();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create member");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel title="Add member">
      <div className="form-row">
        <label>
          Email
          <input
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="person@example.com"
          />
        </label>
        <label>
          Display name
          <input
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            placeholder="Optional"
          />
        </label>
        <label>
          Role
          <select value={role} onChange={(e) => setRole(e.target.value)}>
            {ROLES.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
        <button
          className="primary"
          disabled={busy || !email.trim()}
          onClick={submit}
        >
          {busy ? "Adding..." : "Add member"}
        </button>
      </div>
      {error ? <p className="error-text">{error}</p> : null}
    </Panel>
  );
}

function MemberRow({
  member,
  ownerCount,
  onChanged,
}: {
  member: MembershipItem;
  ownerCount: number;
  onChanged: () => void;
}) {
  const { client } = useSession();
  const api = admin(client);
  const [role, setRole] = useState(member.role);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setRole(member.role);
    setError(null);
  }, [member]);

  const saveRole = async (nextRole: string) => {
    if (member.role === "owner" && ownerCount === 1 && nextRole !== "owner") {
      setError("This workspace must keep at least one owner.");
      return;
    }
    setRole(nextRole);
    setBusy(true);
    setError(null);
    try {
      await api.updateMemberRole(member.user_id, nextRole);
      onChanged();
    } catch (e) {
      setRole(member.role);
      setError(e instanceof Error ? e.message : "Failed to update role");
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (member.role === "owner" && ownerCount === 1) {
      setError("This workspace must keep at least one owner.");
      return;
    }
    if (!window.confirm(`Remove ${member.display_name} from this workspace?`)) return;
    setBusy(true);
    setError(null);
    try {
      await api.removeMember(member.user_id);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to remove member");
    } finally {
      setBusy(false);
    }
  };

  return (
    <tr>
      <td>
        <strong>{member.display_name}</strong>
        {member.role === "owner" && ownerCount === 1 ? (
          <div className="muted small">Last owner</div>
        ) : null}
        {error ? <div className="error-text">{error}</div> : null}
      </td>
      <td>{member.email}</td>
      <td>
        <select value={role} disabled={busy} onChange={(e) => saveRole(e.target.value)}>
          {ROLES.map((item) => (
            <option key={item} value={item}>
              {item}
            </option>
          ))}
        </select>
      </td>
      <td className="row-actions">
        <button className="danger small" disabled={busy} onClick={remove}>
          Remove
        </button>
      </td>
    </tr>
  );
}
