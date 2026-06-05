import type { HowlloClient } from "./client";

// Moderation actions are admin-only and authorized server-side. They resolve
// the tenant from the post itself, so they do not take a tenant_slug query
// param (withTenant: false). All use PATCH to match the server contract.
export const moderation = (client: HowlloClient) => ({
  setVisibility: (postId: string, isHidden: boolean) =>
    client.request<void>(`/api/admin/posts/${postId}/visibility`, {
      method: "PATCH",
      withTenant: false,
      body: JSON.stringify({ is_hidden: isHidden }),
    }),
  setLock: (postId: string, isLocked: boolean) =>
    client.request<void>(`/api/admin/posts/${postId}/lock`, {
      method: "PATCH",
      withTenant: false,
      body: JSON.stringify({ is_locked: isLocked }),
    }),
  setStatus: (postId: string, status: string, reason?: string) =>
    client.request<void>(`/api/admin/posts/${postId}/status`, {
      method: "PATCH",
      withTenant: false,
      body: JSON.stringify({ status, reason }),
    }),
  setDuplicate: (postId: string, duplicateOfPostId: string | null) =>
    client.request<void>(`/api/admin/posts/${postId}/duplicate`, {
      method: "PATCH",
      withTenant: false,
      body: JSON.stringify({ duplicate_of_post_id: duplicateOfPostId }),
    }),
  softDelete: (postId: string) =>
    client.request<void>(`/api/admin/posts/${postId}/soft-delete`, {
      method: "PATCH",
      withTenant: false,
    }),
  /** Restore a hidden or soft-deleted post back to visible. */
  restore: (postId: string) =>
    client.request<void>(`/api/admin/posts/${postId}/restore`, {
      method: "PATCH",
      withTenant: false,
    }),
});
