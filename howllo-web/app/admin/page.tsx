import { redirect } from "next/navigation";
import { BOARD_ADMIN_BASE_URL } from "@/lib/config";
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
    const target = new URL(BOARD_ADMIN_BASE_URL);

    target.searchParams.set("tenant", tenant);

    redirect(target.toString());
  } catch (error) {
    if (error instanceof WorkspaceContextError) {
      return <WorkspaceState kind={error.kind} workspaceSlug={error.workspaceSlug} />;
    }
    throw error;
  }
}
