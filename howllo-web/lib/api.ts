import { API_BASE_URL } from "@/lib/config";
import type {
  ApiTokenListItem,
  Board,
  BoardDetail,
  BootstrapTenant,
  Comment,
  CurrentUser,
  ManageBoard,
  MyActivity,
  MyInvitation,
  WorkspaceMember,
  Notification,
  PaginatedResponse,
  PostDetail,
  PostListItem,
  RoadmapItem,
  StatusHistoryItem,
  Tag,
  TenantBranding,
  WorkspaceAuthConfig,
  WebhookEndpoint,
} from "@/lib/types";

function buildUrl(path: string) {
  return `${API_BASE_URL}${path}`;
}

export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

/**
 * Thrown when the Howllo backend cannot be reached at all (connection refused,
 * DNS failure, timeout) — as opposed to ApiError, which means the server
 * responded with a non-2xx status. Lets callers render a friendly
 * "server unavailable" state instead of crashing on a raw TypeError.
 */
export class ApiUnavailableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ApiUnavailableError";
  }
}

/**
 * fetch() that converts low-level network failures into ApiUnavailableError.
 * Use this instead of the global fetch for every backend call.
 */
async function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  try {
    return await fetch(input, init);
  } catch (error) {
    throw new ApiUnavailableError(
      error instanceof Error ? error.message : "Could not reach the Howllo server.",
    );
  }
}

async function unwrap<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const text = await response.text();
    throw new ApiError(response.status, text || `Request failed with ${response.status}`);
  }
  return response.json() as Promise<T>;
}

export async function getBoards(tenantSlug: string): Promise<Board[]> {
  const response = await apiFetch(
    buildUrl(`/api/boards?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    { cache: "no-store" },
  );
  return unwrap<Board[]>(response);
}

export async function getBootstrapTenant(): Promise<BootstrapTenant> {
  const response = await apiFetch(buildUrl("/api/bootstrap"), { cache: "no-store" });
  return unwrap<BootstrapTenant>(response);
}

export async function getTenantBranding(tenantSlug: string): Promise<TenantBranding> {
  const response = await apiFetch(
    buildUrl(`/api/tenant-branding?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    { cache: "no-store" },
  );
  return unwrap<TenantBranding>(response);
}

export async function getWorkspaceAuthConfig(
  tenantSlug: string,
): Promise<WorkspaceAuthConfig> {
  const response = await apiFetch(
    buildUrl(`/api/workspace-auth?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    { cache: "no-store" },
  );
  return unwrap<WorkspaceAuthConfig>(response);
}

export async function getBoardDetail(
  tenantSlug: string,
  boardSlug: string,
  token?: string,
): Promise<BoardDetail> {
  const headers = token ? { Authorization: token } : undefined;
  const response = await apiFetch(
    buildUrl(
      `/api/boards/${encodeURIComponent(boardSlug)}?tenant_slug=${encodeURIComponent(tenantSlug)}`,
    ),
    { cache: "no-store", headers },
  );
  return unwrap<BoardDetail>(response);
}

export async function getBoardPosts(params: {
  tenantSlug: string;
  boardSlug: string;
  sort?: string;
  status?: string;
  tag?: string;
  page?: number;
  perPage?: number;
  token?: string;
}): Promise<PostListItem[]> {
  const search = new URLSearchParams({
    tenant_slug: params.tenantSlug,
  });
  if (params.sort) search.set("sort", params.sort);
  if (params.status) search.set("status", params.status);
  if (params.tag) search.set("tag", params.tag);
  if (params.page) search.set("page", String(params.page));
  if (params.perPage) search.set("per_page", String(params.perPage));

  const headers = params.token ? { Authorization: params.token } : undefined;
  const response = await apiFetch(
    buildUrl(`/api/boards/${encodeURIComponent(params.boardSlug)}/posts?${search.toString()}`),
    { cache: "no-store", headers },
  );
  const payload = await unwrap<PaginatedResponse<PostListItem>>(response);
  return payload.items;
}

export async function getPostDetail(
  tenantSlug: string,
  postId: string,
  token?: string,
): Promise<PostDetail> {
  const headers = token ? { Authorization: token } : undefined;
  const response = await apiFetch(
    buildUrl(`/api/posts/${postId}?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    { cache: "no-store", headers },
  );
  return unwrap<PostDetail>(response);
}

export async function getComments(postId: string, token?: string): Promise<Comment[]> {
  const headers = token ? { Authorization: token } : undefined;
  const response = await apiFetch(buildUrl(`/api/posts/${postId}/comments`), {
    cache: "no-store",
    headers,
  });
  return unwrap<Comment[]>(response);
}

export async function getStatusHistory(
  tenantSlug: string,
  postId: string,
  token?: string,
): Promise<StatusHistoryItem[]> {
  const headers = token ? { Authorization: token } : undefined;
  const response = await apiFetch(
    buildUrl(`/api/posts/${postId}/status-history?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    {
      cache: "no-store",
      headers,
    },
  );
  return unwrap<StatusHistoryItem[]>(response);
}

export async function getRoadmap(
  tenantSlug: string,
  filters?: { status?: string; tag?: string; token?: string },
): Promise<RoadmapItem[]> {
  const search = new URLSearchParams({
    tenant_slug: tenantSlug,
  });
  if (filters?.status) search.set("status", filters.status);
  if (filters?.tag) search.set("tag", filters.tag);

  const headers = filters?.token ? { Authorization: filters.token } : undefined;
  const response = await apiFetch(buildUrl(`/api/roadmap?${search.toString()}`), {
    cache: "no-store",
    headers,
  });
  return unwrap<RoadmapItem[]>(response);
}

export async function getTags(tenantSlug: string): Promise<Tag[]> {
  const response = await apiFetch(
    buildUrl(`/api/tags?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    { cache: "no-store" },
  );
  return unwrap<Tag[]>(response);
}

export async function getNotifications(token: string): Promise<Notification[]> {
  const response = await apiFetch(buildUrl("/api/notifications"), {
    cache: "no-store",
    headers: {
      Authorization: token,
    },
  });
  return unwrap<Notification[]>(response);
}

// ---- Manager surface (tenant/board-owner) — the admin APIs, called from the
// web with the user's workspace-session token. `Authorization: token` already
// carries the "Bearer " prefix.

export async function getMyWorkspaceRole(tenantSlug: string, token: string): Promise<string | null> {
  const response = await apiFetch(
    buildUrl(`/api/me/workspace-role?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    { cache: "no-store", headers: { Authorization: token } },
  );
  const data = await unwrap<{ role: string | null }>(response);
  return data.role;
}

async function managerWrite(path: string, method: string, token: string, body?: unknown): Promise<void> {
  const response = await apiFetch(buildUrl(path), {
    method,
    headers: { "content-type": "application/json", Authorization: token },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) {
    throw new ApiError(response.status, (await response.text()) || `Request failed with ${response.status}`);
  }
}

export async function managerListBoards(tenantSlug: string, token: string): Promise<ManageBoard[]> {
  const response = await apiFetch(
    buildUrl(`/api/admin/boards?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    { cache: "no-store", headers: { Authorization: token } },
  );
  return unwrap<ManageBoard[]>(response);
}

export async function managerUpdateBoard(
  boardId: string,
  input: {
    name: string;
    description?: string | null;
    board_type: string;
    is_private: boolean;
    background_color?: string | null;
    dashboard_sections?: string[];
    icon_url?: string | null;
  },
  token: string,
): Promise<void> {
  await managerWrite(`/api/admin/boards/${boardId}`, "PATCH", token, input);
}

export async function managerListInvitations(tenantSlug: string, token: string): Promise<MyInvitation[]> {
  const response = await apiFetch(
    buildUrl(`/api/admin/invitations?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    { cache: "no-store", headers: { Authorization: token } },
  );
  return unwrap<MyInvitation[]>(response);
}

export async function managerCreateInvitation(
  tenantSlug: string,
  input: { email: string; role: string },
  token: string,
): Promise<void> {
  await managerWrite(`/api/admin/invitations`, "POST", token, { ...input, tenant_slug: tenantSlug });
}

export async function managerWithdrawInvitation(id: string, tenantSlug: string, token: string): Promise<void> {
  await managerWrite(
    `/api/admin/invitations/${id}/withdraw?tenant_slug=${encodeURIComponent(tenantSlug)}`,
    "POST",
    token,
  );
}

export async function managerListMembers(tenantSlug: string, token: string): Promise<WorkspaceMember[]> {
  const response = await apiFetch(
    buildUrl(`/api/admin/members?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    { cache: "no-store", headers: { Authorization: token } },
  );
  return unwrap<WorkspaceMember[]>(response);
}

export async function managerUpdateMemberRole(
  userId: string,
  role: string,
  tenantSlug: string,
  token: string,
): Promise<void> {
  await managerWrite(
    `/api/admin/members/${userId}/role?tenant_slug=${encodeURIComponent(tenantSlug)}`,
    "PATCH",
    token,
    { role },
  );
}

export async function managerRemoveMember(userId: string, tenantSlug: string, token: string): Promise<void> {
  await managerWrite(
    `/api/admin/members/${userId}?tenant_slug=${encodeURIComponent(tenantSlug)}`,
    "DELETE",
    token,
  );
}

export async function getUnreadCount(token: string): Promise<number> {
  const response = await apiFetch(buildUrl("/api/notifications/unread-count"), {
    cache: "no-store",
    headers: { Authorization: token },
  });
  const data = await unwrap<{ unread: number }>(response);
  return data.unread;
}

export async function markAllNotificationsRead(token: string): Promise<void> {
  const response = await apiFetch(buildUrl("/api/notifications/read-all"), {
    method: "POST",
    headers: { Authorization: token },
  });
  if (!response.ok) {
    throw new ApiError(response.status, (await response.text()) || `Request failed with ${response.status}`);
  }
}

export async function markNotificationRead(id: string, token: string): Promise<void> {
  const response = await apiFetch(buildUrl(`/api/notifications/${id}/read`), {
    method: "PATCH",
    headers: { Authorization: token },
  });
  if (!response.ok) {
    throw new ApiError(response.status, (await response.text()) || `Request failed with ${response.status}`);
  }
}

export async function getMe(token: string): Promise<CurrentUser> {
  const response = await apiFetch(buildUrl("/api/me"), {
    cache: "no-store",
    headers: {
      Authorization: token,
    },
  });
  return unwrap<CurrentUser>(response);
}

export async function createWorkspaceSession(input: {
  tenantSlug: string;
  rooiamAccessToken: string;
}): Promise<{ session_token: string }> {
  const response = await apiFetch(buildUrl("/api/auth/workspace-session"), {
    method: "POST",
    headers: {
      "content-type": "application/json",
      Authorization: `Bearer ${input.rooiamAccessToken}`,
    },
    body: JSON.stringify({
      tenant_slug: input.tenantSlug,
    }),
  });
  return unwrap<{ session_token: string }>(response);
}

export async function revokeWorkspaceSession(token: string): Promise<void> {
  const response = await apiFetch(buildUrl("/api/auth/workspace-session"), {
    method: "DELETE",
    headers: {
      Authorization: token,
    },
  });
  if (!response.ok && response.status !== 401) {
    const text = await response.text();
    throw new ApiError(response.status, text || `Request failed with ${response.status}`);
  }
}

export async function updateMe(input: {
  token: string;
  displayName: string;
  avatarUrl?: string | null;
}): Promise<CurrentUser> {
  const response = await apiFetch(buildUrl("/api/me"), {
    method: "PATCH",
    headers: {
      "content-type": "application/json",
      Authorization: input.token,
    },
    body: JSON.stringify({
      display_name: input.displayName,
      avatar_url: input.avatarUrl ?? null,
    }),
  });
  return unwrap<CurrentUser>(response);
}

export async function getMyActivity(
  tenantSlug: string,
  token: string,
): Promise<MyActivity> {
  const response = await apiFetch(
    buildUrl(`/api/me/activity?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    {
      cache: "no-store",
      headers: {
        Authorization: token,
      },
    },
  );
  return unwrap<MyActivity>(response);
}

export async function getMyInvitations(token: string): Promise<MyInvitation[]> {
  const response = await apiFetch(buildUrl("/api/me/invitations"), {
    cache: "no-store",
    headers: {
      Authorization: token,
    },
  });
  return unwrap<MyInvitation[]>(response);
}

export async function respondToInvitation(
  invitationId: string,
  action: "accept" | "reject",
  token: string,
): Promise<void> {
  const response = await apiFetch(
    buildUrl(`/api/me/invitations/${invitationId}/${action}`),
    {
      method: "POST",
      headers: {
        Authorization: token,
      },
    },
  );
  if (!response.ok) {
    throw new ApiError(response.status, (await response.text()) || `Request failed with ${response.status}`);
  }
}

export async function getWebhooks(
  tenantSlug: string,
  token: string,
): Promise<WebhookEndpoint[]> {
  const response = await apiFetch(
    buildUrl(`/api/admin/webhooks?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    {
      cache: "no-store",
      headers: {
        Authorization: token,
      },
    },
  );
  return unwrap<WebhookEndpoint[]>(response);
}

export async function getApiTokens(
  tenantSlug: string,
  token: string,
): Promise<ApiTokenListItem[]> {
  const response = await apiFetch(
    buildUrl(`/api/admin/api-tokens?tenant_slug=${encodeURIComponent(tenantSlug)}`),
    {
      cache: "no-store",
      headers: {
        Authorization: token,
      },
    },
  );
  return unwrap<ApiTokenListItem[]>(response);
}

export async function createPost(input: {
  tenantSlug: string;
  boardSlug: string;
  title: string;
  body: string;
  attachments?: string[];
  token: string;
}) {
  const response = await apiFetch(
    buildUrl(`/api/boards/${encodeURIComponent(input.boardSlug)}/posts`),
    {
      method: "POST",
      headers: {
        "content-type": "application/json",
        Authorization: input.token,
      },
      body: JSON.stringify({
        tenant_slug: input.tenantSlug,
        title: input.title,
        body: input.body,
        attachments: input.attachments ?? [],
      }),
    },
  );
  return unwrap<{ id: string }>(response);
}

/** Upload an image (base64) and return its public URL. */
export async function uploadImage(file: File, token: string): Promise<string> {
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("Could not read file"));
    reader.readAsDataURL(file);
  });
  const base64 = dataUrl.split(",")[1] ?? "";
  const response = await apiFetch(buildUrl(`/api/uploads`), {
    method: "POST",
    headers: { "content-type": "application/json", Authorization: token },
    body: JSON.stringify({
      filename: file.name,
      content_type: file.type,
      data: base64,
    }),
  });
  const result = await unwrap<{ url: string }>(response);
  return result.url;
}

export async function createComment(input: {
  postId: string;
  body: string;
  token: string;
}) {
  const response = await apiFetch(buildUrl(`/api/posts/${input.postId}/comments`), {
    method: "POST",
    headers: {
      "content-type": "application/json",
      Authorization: input.token,
    },
    body: JSON.stringify({
      body: input.body,
    }),
  });
  return unwrap<{ id: string }>(response);
}

export async function votePost(postId: string, token: string) {
  const response = await apiFetch(buildUrl(`/api/posts/${postId}/vote`), {
    method: "POST",
    headers: {
      Authorization: token,
    },
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
}

export async function followPost(postId: string, token: string) {
  const response = await apiFetch(buildUrl(`/api/posts/${postId}/follow`), {
    method: "POST",
    headers: {
      Authorization: token,
    },
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
}
