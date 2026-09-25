import { redirect } from "next/navigation";

export default async function TenantManagePage({ params }: { params: Promise<{ tenantSlug: string }> }) {
  const { tenantSlug } = await params;
  redirect(`/app/${encodeURIComponent(tenantSlug)}`);
}
