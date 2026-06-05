import type {
  ApiTokenCreated,
  ApiTokenListItem,
  AdminTenantSummary,
  AuditLogItem,
  BoardDetail,
  BoardSummary,
  MembershipItem,
  PaginatedResponse,
  Tag,
  TenantBranding,
  WebhookDelivery,
  WebhookEndpoint,
  PlatformStorageConfig,
  PlatformStorageConfigUpdate,
  TestStorageRequest,
  StorageTestResult,
  PlatformDatabaseInfo,
  WorkspaceStorageUsageResponse,
  PlatformStatusResponse,
  BuildInfoResponse,
} from "@howllo/types";
import type { HowlloClient } from "./client";

// Admin-only reads/writes. Tenant is supplied via the client (tenant_slug query),
// and also echoed in POST bodies where the server expects it.
export const admin = (client: HowlloClient) => ({
  // Platform
  listTenants: () => client.request<AdminTenantSummary[]>(`/api/admin/tenants`, {
    withTenant: false,
  }),

  createTenant: (input: { name: string; slug?: string }) =>
    client.request<AdminTenantSummary>(`/api/admin/tenants`, {
      method: "POST",
      withTenant: false,
      body: JSON.stringify(input),
    }),

  // Permanently delete a workspace. `confirmSlug` must equal the slug — the
  // server re-checks it as a typed confirmation.
  deleteTenant: (slug: string, confirmSlug: string) =>
    client.request<void>(`/api/admin/tenants/${encodeURIComponent(slug)}`, {
      method: "DELETE",
      withTenant: false,
      body: JSON.stringify({ confirm_slug: confirmSlug }),
    }),

  // Boards
  listBoards: () => client.request<BoardDetail[]>(`/api/admin/boards`),

  createBoard: (input: {
    slug: string;
    name: string;
    description?: string;
    board_type: string;
    is_private: boolean;
    icon_url?: string | null;
  }) =>
    client.request<BoardDetail>(`/api/admin/boards`, {
      method: "POST",
      withTenant: false,
      body: JSON.stringify({ ...input, tenant_slug: client.tenant }),
    }),

  updateBoard: (
    boardId: string,
    input: {
      name: string;
      description?: string;
      board_type: string;
      is_private: boolean;
      icon_url?: string | null;
    },
  ) =>
    client.request<BoardDetail>(`/api/admin/boards/${boardId}`, {
      method: "PATCH",
      withTenant: false,
      body: JSON.stringify(input),
    }),

  deleteBoard: (boardId: string) =>
    client.request<void>(`/api/admin/boards/${boardId}`, {
      method: "DELETE",
      withTenant: false,
    }),

  getBoardSummary: (boardId: string) =>
    client.request<BoardSummary>(`/api/admin/boards/${boardId}/summary`),

  // Tags
  listTags: () => client.request<Tag[]>(`/api/tags`),

  createTag: (input: { slug: string; name: string; color?: string }) =>
    client.request<Tag>(`/api/admin/tags`, {
      method: "POST",
      withTenant: false,
      body: JSON.stringify({ ...input, tenant_slug: client.tenant }),
    }),

  updateTag: (tagId: string, input: { name: string; color?: string }) =>
    client.request<Tag>(`/api/admin/tags/${tagId}`, {
      method: "PATCH",
      withTenant: false,
      body: JSON.stringify(input),
    }),

  deleteTag: (tagId: string) =>
    client.request<void>(`/api/admin/tags/${tagId}`, {
      method: "DELETE",
      withTenant: false,
    }),

  // Members
  listMembers: () => client.request<MembershipItem[]>(`/api/admin/members`),

  createMember: (input: {
    email: string;
    display_name?: string;
    role: string;
  }) =>
    client.request<MembershipItem>(`/api/admin/members`, {
      method: "POST",
      withTenant: false,
      body: JSON.stringify({ ...input, tenant_slug: client.tenant }),
    }),

  updateMemberRole: (userId: string, role: string) =>
    client.request<MembershipItem>(`/api/admin/members/${userId}/role`, {
      method: "PATCH",
      body: JSON.stringify({ role }),
    }),

  removeMember: (userId: string) =>
    client.request<void>(`/api/admin/members/${userId}`, {
      method: "DELETE",
    }),

  // Audit
  listAuditLogs: (page = 1, perPage = 20) =>
    client.request<PaginatedResponse<AuditLogItem>>(
      `/api/admin/audit-logs?page=${page}&per_page=${perPage}`,
    ),

  // Webhooks
  listWebhooks: () =>
    client.request<WebhookEndpoint[]>(`/api/admin/webhooks`),

  createWebhook: (input: { url: string; secret?: string }) =>
    client.request<WebhookEndpoint>(`/api/admin/webhooks`, {
      method: "POST",
      withTenant: false,
      body: JSON.stringify({ ...input, tenant_slug: client.tenant }),
    }),

  deactivateWebhook: (webhookId: string) =>
    client.request<void>(`/api/admin/webhooks/${webhookId}/deactivate`, {
      method: "PATCH",
      withTenant: false,
    }),

  listWebhookDeliveries: () =>
    client.request<WebhookDelivery[]>(`/api/admin/webhooks/deliveries`),

  resendWebhookDelivery: (deliveryId: string) =>
    client.request<void>(`/api/admin/webhooks/deliveries/${deliveryId}/resend`, {
      method: "POST",
      withTenant: false,
      body: JSON.stringify({ tenant_slug: client.tenant }),
    }),

  // API tokens
  listApiTokens: () =>
    client.request<ApiTokenListItem[]>(`/api/admin/api-tokens`),

  createApiToken: (input: { name: string; scopes?: string[] }) =>
    client.request<ApiTokenCreated>(`/api/admin/api-tokens`, {
      method: "POST",
      withTenant: false,
      body: JSON.stringify({ ...input, tenant_slug: client.tenant }),
    }),

  revokeApiToken: (tokenId: string) =>
    client.request<void>(`/api/admin/api-tokens/${tokenId}/revoke`, {
      method: "PATCH",
      withTenant: false,
    }),

  // Exports
  exportPostsJson: () => client.request<unknown[]>(`/api/admin/export/posts.json`),

  exportPostsCsv: () => client.requestText(`/api/admin/export/posts.csv`),

  // Branding
  getTenantBranding: () => client.request<TenantBranding>(`/api/tenant-branding`),

  updateTenantBranding: (input: {
    site_name?: string;
    logo_url?: string;
    accent_color?: string;
    show_powered_by: boolean;
  }) =>
    client.request<TenantBranding>(`/api/admin/tenant-branding`, {
      method: "PATCH",
      body: JSON.stringify(input),
    }),

  // Platform settings (local-admin only)
  getStorageConfig: () =>
    client.request<PlatformStorageConfig>(`/api/admin/platform/storage`, {
      withTenant: false,
    }),

  getPlatformStatus: () =>
    client.request<PlatformStatusResponse>(`/api/admin/platform/status`, {
      withTenant: false,
    }),

  getBuildInfo: () =>
    client.request<BuildInfoResponse>(`/api/admin/platform/build`, {
      withTenant: false,
    }),

  getWorkspaceStorageUsage: () =>
    client.request<WorkspaceStorageUsageResponse>(
      `/api/admin/platform/storage/usage`,
      { withTenant: false },
    ),

  saveStorageConfig: (input: PlatformStorageConfigUpdate) =>
    client.request<PlatformStorageConfig>(`/api/admin/platform/storage`, {
      method: "POST",
      withTenant: false,
      body: JSON.stringify(input),
    }),

  testStorage: (input: TestStorageRequest) =>
    client.request<StorageTestResult>(`/api/admin/platform/storage/test`, {
      method: "POST",
      withTenant: false,
      body: JSON.stringify(input),
    }),

  testDatabase: () =>
    client.request<PlatformDatabaseInfo>(`/api/admin/platform/database/test`, {
      method: "POST",
      withTenant: false,
    }),
});
