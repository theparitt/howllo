import { redirect } from "next/navigation";

type TenantRootPageProps = {
  params: Promise<{
    tenantSlug: string;
  }>;
};

export default async function TenantRootPage({ params }: TenantRootPageProps) {
  const { tenantSlug } = await params;
  redirect(`/${encodeURIComponent(tenantSlug)}/dashboard`);
}
