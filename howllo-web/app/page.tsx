import { redirect } from "next/navigation";
import {
  WorkspaceContextError,
  buildTenantPath,
  resolveTenantContext,
} from "@/lib/default-tenant";
import { WorkspaceState } from "@/components/workspace-state";

type HomePageProps = {
  searchParams: Promise<{
    tenant?: string;
  }>;
};

export default async function HomePage({ searchParams }: HomePageProps) {
  const params = await searchParams;
  try {
    const { tenantSlug, defaultTenantSlug } = await resolveTenantContext(params.tenant);

    redirect(buildTenantPath("/dashboard", tenantSlug, defaultTenantSlug));
  } catch (error) {
    if (error instanceof WorkspaceContextError) {
      return (
        <WorkspaceState kind={error.kind} workspaceSlug={error.workspaceSlug} />
      );
    }
    throw error;
  }
}
