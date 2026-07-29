export const LEGACY_DEV_AUTH_COOKIE = "howllo_dev_bearer_token";

export function sanitizeWorkspaceSlugForCookie(tenantSlug: string) {
  return tenantSlug.trim().replace(/[^A-Za-z0-9_-]/g, "_");
}

export function getWorkspaceAuthCookieName(tenantSlug: string) {
  return `howllo_auth_${sanitizeWorkspaceSlugForCookie(tenantSlug)}`;
}
