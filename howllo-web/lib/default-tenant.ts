import { ApiError, getTenantBranding } from "@/lib/api";

export type TenantContext = {
  tenantSlug: string;
  defaultTenantSlug: string;
  isDefaultTenant: boolean;
};

export class WorkspaceContextError extends Error {
  readonly kind: "missing" | "invalid";
  readonly workspaceSlug?: string;

  constructor(kind: "missing" | "invalid", workspaceSlug?: string) {
    super(
      kind === "missing"
        ? "No workspace parameter was provided."
        : `There is no workspace with slug or id "${workspaceSlug}".`,
    );
    this.name = "WorkspaceContextError";
    this.kind = kind;
    this.workspaceSlug = workspaceSlug;
  }
}

export async function resolveTenantSlug(preferred?: string): Promise<string> {
  return (await resolveTenantContext(preferred)).tenantSlug;
}

export async function resolveTenantContext(preferred?: string): Promise<TenantContext> {
  const tenantSlug = preferred?.trim();
  if (!tenantSlug) {
    throw new WorkspaceContextError("missing");
  }

  try {
    await getTenantBranding(tenantSlug);
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) {
      throw new WorkspaceContextError("invalid", tenantSlug);
    }
    throw error;
  }

  return {
    tenantSlug,
    defaultTenantSlug: tenantSlug,
    isDefaultTenant: true,
  };
}

export function buildTenantPath(
  path: string,
  tenantSlug: string,
  _defaultTenantSlug: string,
  search?: URLSearchParams,
): string {
  const params = new URLSearchParams(search);
  params.delete("tenant");

  const query = params.toString();
  const normalizedPath = path === "/" ? "" : path;
  const basePath = `/${encodeURIComponent(tenantSlug)}${normalizedPath}`;
  return query ? `${basePath}?${query}` : basePath;
}
