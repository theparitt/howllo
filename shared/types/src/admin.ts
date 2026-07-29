export type PaginatedResponse<T> = {
  items: T[];
  page: number;
  per_page: number;
  total: number;
  has_next: boolean;
};

export type AuditLogItem = {
  id: string;
  tenant_id: string;
  actor_user_id: string;
  actor_display_name: string;
  entity_type: string;
  entity_id: string;
  action: string;
  old_value: unknown;
  new_value: unknown;
  reason: string | null;
  request_id: string | null;
  created_at: string;
};

export type MembershipItem = {
  user_id: string;
  email: string;
  display_name: string;
  role: string;
};

export type WorkspaceAuthConfig = {
  tenant_slug: string;
  provider: string;
  rooiam_workspace_id: string | null;
  rooiam_client_id: string | null;
  rooiam_widget_base_url: string | null;
};
