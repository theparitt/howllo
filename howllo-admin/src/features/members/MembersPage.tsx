import { useEffect, useState } from "react";
import { admin } from "@howllo/api-client";
import type { InvitationItem, MembershipItem } from "@howllo/types";
import { Panel } from "../../components/Panel";
import { SessionRequired } from "../../components/SessionRequired";
import { StateBlock } from "../../components/StateBlock";
import { useSession } from "../../lib/session";
import { useAsync } from "../../lib/useAsync";

const ROLES = ["owner", "admin", "moderator", "member"];
// Roles that can be invited (owner is granted only at workspace creation).
const INVITE_ROLES = ["admin", "moderator", "member"];

function statusTone(status: string): "green" | "blue" | "warm" | "violet" {
  switch (status) {
    case "accepted":
      return "green";
    case "pending":
      return "violet";
    case "rejected":
    case "withdrawn":
    case "expired":
      return "warm";
    default:
      return "blue";
  }
}

export function MembersPage() {
  const { client, tenant, authorization } = useSession();
  const api = admin(client);
  const members = useAsync<MembershipItem[]>(
    () => (authorization ? api.listMembers() : Promise.resolve([])),
    [tenant, authorization],
  );
  const invitations = useAsync<InvitationItem[]>(
    () => (authorization ? api.listInvitations() : Promise.resolve([])),
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
      <p className="muted">
        Invite staff by email and manage role boundaries for this workspace.
      </p>

      <InviteStaff onInvited={invitations.reload} />

      <Panel title="Invitations">
        <StateBlock
          loading={invitations.loading}
          error={invitations.error}
          empty={invitations.data?.length === 0 ? "No invitations yet." : null}
        >
          <table className="data-table">
            <thead>
              <tr>
                <th>Email</th>
                <th>Role</th>
                <th>Status</th>
                <th>Invited by</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {invitations.data?.map((invite) => (
                <InvitationRow
                  key={invite.id}
                  invite={invite}
                  onChanged={invitations.reload}
                />
              ))}
            </tbody>
          </table>
        </StateBlock>
      </Panel>

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

function InviteStaff({ onInvited }: { onInvited: () => void }) {
  const { client } = useSession();
  const api = admin(client);
  const [email, setEmail] = useState("");
  const [role, setRole] = useState("moderator");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await api.createInvitation({ email: email.trim(), role });
      setNotice(`Invitation sent to ${email.trim()}.`);
      setEmail("");
      setRole("moderator");
      onInvited();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to send invitation");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel title="Invite staff">
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
          Role
          <select value={role} onChange={(e) => setRole(e.target.value)}>
            {INVITE_ROLES.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
        <button type="button" className="primary" disabled={busy || !email.trim()} onClick={submit}>
          {busy ? "Sending..." : "Send invite"}
        </button>
      </div>
      <p className="muted small">
        The person accepts (or declines) the invitation the next time they sign in.
      </p>
      {notice ? <p className="success-text">{notice}</p> : null}
      {error ? <p className="error-text">{error}</p> : null}
    </Panel>
  );
}

function InvitationRow({
  invite,
  onChanged,
}: {
  invite: InvitationItem;
  onChanged: () => void;
}) {
  const { client } = useSession();
  const api = admin(client);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const withdraw = async () => {
    if (!window.confirm(`Withdraw the invitation for ${invite.email}?`)) return;
    setBusy(true);
    setError(null);
    try {
      await api.withdrawInvitation(invite.id);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to withdraw invitation");
    } finally {
      setBusy(false);
    }
  };

  return (
    <tr>
      <td>
        {invite.email}
        {error ? <div className="error-text">{error}</div> : null}
      </td>
      <td>{invite.role}</td>
      <td>
        <span className="badge" data-tone={statusTone(invite.status)}>
          {invite.status}
        </span>
      </td>
      <td className="muted">{invite.invited_by_name ?? "—"}</td>
      <td className="row-actions">
        {invite.status === "pending" ? (
          <button type="button" className="danger small" disabled={busy} onClick={withdraw}>
            Withdraw
          </button>
        ) : null}
      </td>
    </tr>
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
