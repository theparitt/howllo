import { redirect } from "next/navigation";
import { getBootstrapTenant } from "@/lib/api";
import { ApiUnavailable } from "@/components/api-unavailable";

type HomePageProps = {
  searchParams: Promise<{
    tenant?: string;
  }>;
};

// Public web root opens a board directory. Tenant management lives in howllo-app.
export default async function HomePage({ searchParams }: HomePageProps) {
  const { tenant } = await searchParams;
  if (tenant?.trim()) {
    redirect(`/${encodeURIComponent(tenant.trim())}`);
  }
  let defaultTenantSlug: string;
  try {
    ({ default_tenant_slug: defaultTenantSlug } = await getBootstrapTenant());
  } catch (error) {
    return <ApiUnavailable message={error instanceof Error ? error.message : "Could not load the default workspace."} />;
  }
  redirect(`/${encodeURIComponent(defaultTenantSlug)}`);
}
