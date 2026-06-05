import { Panel } from "./Panel";

export function SessionRequired() {
  return (
    <Panel title="Workspace required">
      <p className="muted">
        Choose a workspace from the sidebar switcher or from the Workspaces
        screen to load workspace-level admin data.
      </p>
    </Panel>
  );
}
