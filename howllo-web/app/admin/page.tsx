import { redirect } from "next/navigation";
import { WorkspaceContextError, resolveTenantSlug } from "@/lib/default-tenant";
import { WorkspaceState } from "@/components/workspace-state";

type AdminPageProps = {
  searchParams: Promise<{
    tenant?: string;
  }>;
};

export default async function AdminPage({ searchParams }: AdminPageProps) {
  const params = await searchParams;
  try {
    const tenant = await resolveTenantSlug(params.tenant);
    redirect(`/app/${encodeURIComponent(tenant)}`);
  } catch (error) {
    if (error instanceof WorkspaceContextError) {
      return <WorkspaceState kind={error.kind} workspaceSlug={error.workspaceSlug} />;
    }
    throw error;
  }
}
