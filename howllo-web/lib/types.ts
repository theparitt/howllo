export type Board = {
  id: string;
  slug: string;
  name: string;
  description: string | null;
  board_type: string;
  icon_url: string | null;
};

export type BootstrapTenant = {
  default_tenant_slug: string;
  default_tenant_name: string;
};

export type TenantBranding = {
  tenant_slug: string;
  tenant_name: string;
  site_name: string;
  logo_url: string | null;
  accent_color: string | null;
  show_powered_by: boolean;
};

export type PaginatedResponse<T> = {
  items: T[];
  page: number;
  per_page: number;
  total: number;
  has_next: boolean;
};

export type BoardDetail = Board & {
  is_private: boolean;
};

export type PostListItem = {
  id: string;
  title: string;
  status: string;
  vote_count: number;
  comment_count: number;
  duplicate_of_post_id: string | null;
  created_at: string;
};

export type PostDetail = {
  id: string;
  title: string;
  body: string;
  status: string;
  vote_count: number;
  is_locked: boolean;
  duplicate_of_post_id: string | null;
  created_at: string;
  board_slug: string;
  attachments?: string[];
};

export type Comment = {
  id: string;
  body: string;
  is_official_response: boolean;
  comment_type: string;
  created_at: string;
  display_name: string;
};

export type RoadmapItem = PostListItem;

export type Tag = {
  id: string;
  slug: string;
  name: string;
  color: string | null;
};

export type StatusHistoryItem = {
  id: string;
  old_status: string | null;
  new_status: string;
  reason: string | null;
  actor_display_name: string;
  created_at: string;
};

export type Notification = {
  id: string;
  event_type: string;
  title: string;
  body: string;
  is_read: boolean;
  post_id: string | null;
  created_at: string;
};

export type WebhookEndpoint = {
  id: string;
  url: string;
  secret: string | null;
  is_active: boolean;
  created_at: string;
};

export type ApiTokenListItem = {
  id: string;
  name: string;
  token_prefix: string;
  revoked_at: string | null;
  created_at: string;
};
