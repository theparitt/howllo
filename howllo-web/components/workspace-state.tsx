type WorkspaceStateProps = {
  kind: "missing" | "invalid";
  workspaceSlug?: string;
};

export function WorkspaceState({ kind, workspaceSlug }: WorkspaceStateProps) {
  return (
    <div className="page-stack">
      <section className="panel empty-state workspace-state">
        <span className="kicker">Workspace required</span>
        <h1 className="empty-state__title">
          {kind === "missing" ? "No workspace parameter was provided." : "Workspace not found."}
        </h1>
        <p className="empty-state__copy">
          {kind === "missing"
            ? "Open Howllo with a workspace URL such as /your-workspace/dashboard or /your-workspace/boards/general."
            : `There is no workspace with slug or id "${workspaceSlug}". Check the workspace URL and try again.`}
        </p>
        <p className="workspace-state__hint">
          Example: <code>/howllo-6soj/dashboard</code>
        </p>
      </section>
    </div>
  );
}
