// Shared route builders so admin and web agree on board identity URLs.
// Board identity is always tenant.slug + board.slug.

export const publicRoutes = {
  workspace: (tenant: string) => `/${tenant}`,
  board: (tenant: string, boardSlug: string) =>
    `/${tenant}/boards/${boardSlug}`,
  post: (tenant: string, boardSlug: string, postId: string) =>
    `/${tenant}/boards/${boardSlug}/posts/${postId}`,
  roadmap: (tenant: string) => `/${tenant}/roadmap`,
  changelog: (tenant: string) => `/${tenant}/changelog`,
};

export const adminRoutes = {
  home: () => `/admin`,
  tenants: () => `/admin/tenants`,
  tenantDetail: (workspaceSlug: string) =>
    `/admin/tenants/${encodeURIComponent(workspaceSlug)}`,
  boards: () => `/admin/boards`,
  board: (boardId: string) => `/admin/boards/${boardId}`,
  tags: () => `/admin/tags`,
  members: () => `/admin/members`,
  audit: () => `/admin/audit`,
  integrations: () => `/admin/integrations`,
  roadmap: () => `/admin/roadmap`,
  moderation: () => `/admin/moderation`,
  settings: () => `/admin/settings`,
  platform: () => `/admin/platform`,
};
