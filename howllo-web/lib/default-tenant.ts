import { getBootstrapTenant } from "@/lib/api";
import { DEFAULT_TENANT } from "@/lib/config";

export type TenantContext = {
  tenantSlug: string;
  defaultTenantSlug: string;
  isDefaultTenant: boolean;
};

export async function resolveTenantSlug(preferred?: string): Promise<string> {
  return (await resolveTenantContext(preferred)).tenantSlug;
}

export async function resolveTenantContext(preferred?: string): Promise<TenantContext> {
  let defaultTenantSlug = DEFAULT_TENANT;

  try {
    const bootstrap = await getBootstrapTenant();
    if (bootstrap.default_tenant_slug.trim()) {
      defaultTenantSlug = bootstrap.default_tenant_slug.trim();
    }
  } catch {}

  const tenantSlug = preferred?.trim() || defaultTenantSlug;

  return {
    tenantSlug,
    defaultTenantSlug,
    isDefaultTenant: tenantSlug === defaultTenantSlug,
  };
}

export function buildTenantPath(
  path: string,
  tenantSlug: string,
  defaultTenantSlug: string,
  search?: URLSearchParams,
): string {
  const params = new URLSearchParams(search);
  params.delete("tenant");

  const query = params.toString();
  const normalizedPath = path === "/" ? "" : path;
  const basePath =
    tenantSlug === defaultTenantSlug ? (normalizedPath || "/") : `/${encodeURIComponent(tenantSlug)}${normalizedPath}`;
  return query ? `${basePath}?${query}` : basePath;
}
