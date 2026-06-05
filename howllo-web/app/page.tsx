import { redirect } from "next/navigation";
import { buildTenantPath, resolveTenantContext } from "@/lib/default-tenant";

type HomePageProps = {
  searchParams: Promise<{
    tenant?: string;
  }>;
};

export default async function HomePage({ searchParams }: HomePageProps) {
  const params = await searchParams;
  const { tenantSlug, defaultTenantSlug } = await resolveTenantContext(params.tenant);

  redirect(buildTenantPath("/dashboard", tenantSlug, defaultTenantSlug));
}
