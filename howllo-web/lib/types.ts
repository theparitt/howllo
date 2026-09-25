export type DashboardSection = "progress" | "latest" | "top";

export type Board = {
  id: string;
  slug: string;
  name: string;
  description: string | null;
  board_type: string;
  icon_url: string | null;
  background_color: string | null;
  dashboard_sections: DashboardSection[];
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
  background_color: string | null;
  show_powered_by: boolean;
  show_roadmap: boolean;
  show_boards: boolean;
  show_feed: boolean;
};

export type WorkspaceAuthConfig = {
  tenant_slug: string;
  provider: string;
  rooiam_workspace_id: string | null;
  rooiam_client_id: string | null;
  rooiam_widget_base_url: string | null;
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
  is_enabled: boolean;
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

export type CurrentUser = {
  id: string;
  rooiam_subject: string | null;
  email: string;
  display_name: string;
  avatar_url: string | null;
};

export type MyPostActivityItem = {
  id: string;
  title: string;
  status: string;
  vote_count: number;
  comment_count: number;
  board_slug: string;
  board_name: string;
  created_at: string;
};

export type MyCommentActivityItem = {
  id: string;
  post_id: string;
  post_title: string;
  board_slug: string;
  board_name: string;
  body: string;
  comment_type: string;
  created_at: string;
};

export type MyPostReferenceActivityItem = {
  post_id: string;
  post_title: string;
  board_slug: string;
  board_name: string;
  created_at: string;
};

export type MyStatusActivityItem = {
  id: string;
  post_id: string;
  post_title: string;
  board_slug: string;
  board_name: string;
  old_status: string | null;
  new_status: string;
  reason: string | null;
  created_at: string;
};

export type MyActivity = {
  posts: MyPostActivityItem[];
  comments: MyCommentActivityItem[];
  votes: MyPostReferenceActivityItem[];
  follows: MyPostReferenceActivityItem[];
  status_changes: MyStatusActivityItem[];
};

export type MyInvitation = {
  id: string;
  email: string;
  role: string;
  status: string;
  tenant_slug: string;
  tenant_name: string;
  invited_by_name: string | null;
  created_at: string;
};

export type WorkspaceParticipant = {
  user_id: string;
  display_name: string;
  email: string;
  joined_at: string;
  restriction_kind: "suspended" | "banned" | null;
  restriction_reason: string | null;
  restriction_expires_at: string | null;
};

export type ModerationQueueItem = {
  id: string;
  title: string;
  board_slug: string;
  status: string;
  is_hidden: boolean;
  author_display_name: string;
  created_at: string;
};

export type ManageBoard = {
  id: string;
  slug: string;
  name: string;
  description: string | null;
  board_type: string;
  is_private: boolean;
  is_enabled: boolean;
  icon_url: string | null;
  background_color: string | null;
  dashboard_sections: DashboardSection[];
};

export type WorkspaceMember = {
  user_id: string;
  email: string;
  display_name: string;
  role: string;
};

export type WorkspaceSummary = {
  id: string;
  slug: string;
  name: string;
  board_count: number;
  member_count: number;
  created_at: string;
  updated_at: string;
};
