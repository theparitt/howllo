import { adminRoutes } from "@howllo/config";
import { useNavigate } from "react-router-dom";
import { useSession } from "../lib/session";

export function SessionBar() {
  const navigate = useNavigate();
  const { tenant, user, logout } = useSession();

  return (
    <header className="session-bar">
      <span className="session-context">
        <span className="session-context-dot" />
        {tenant ? (
          <>
            Workspace <strong>{tenant}</strong>
          </>
        ) : (
          "No workspace selected"
        )}
      </span>
      <div className="session-spacer" />
      <span className="session-user">
        <strong>{user?.name || "Administrator"}</strong>
        <span>{user?.email || ""}</span>
      </span>
      <div className="button-row">
        <button onClick={() => navigate(adminRoutes.tenants())}>Workspaces</button>
      </div>
      <button className="session-logout" onClick={logout}>
        Log out
      </button>
    </header>
  );
}
