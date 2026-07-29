import { redirect } from "next/navigation";
import {
  WorkspaceContextError,
  buildTenantPath,
  resolveTenantContext,
} from "@/lib/default-tenant";
import { getServerBearerToken } from "@/lib/server-auth";
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

    // Signed-in users land on their activity feed; everyone else on the boards.
    const signedIn = Boolean(await getServerBearerToken(tenantSlug));
    const home = signedIn ? "/feed" : "/dashboard";
    redirect(buildTenantPath(home, tenantSlug, defaultTenantSlug));
  } catch (error) {
    if (error instanceof WorkspaceContextError) {
      return (
        <WorkspaceState kind={error.kind} workspaceSlug={error.workspaceSlug} />
      );
    }
    throw error;
  }
}
