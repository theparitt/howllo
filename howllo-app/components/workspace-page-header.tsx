export function WorkspacePageHeader({ title, description, actions }: {
  title: string;
  description: string;
  actions?: React.ReactNode;
}) {
  return <header className="manage-page-header">
    <div className="manage-page-header__copy">
      <h1 className="page-title">{title}</h1>
      <p>{description}</p>
    </div>
    {actions ? <div className="manage-page-header__actions">{actions}</div> : null}
  </header>;
}
