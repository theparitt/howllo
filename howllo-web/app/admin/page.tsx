import { redirect } from "next/navigation";
import { BOARD_ADMIN_BASE_URL } from "@/lib/config";
import { resolveTenantSlug } from "@/lib/default-tenant";

type AdminPageProps = {
  searchParams: Promise<{
    tenant?: string;
  }>;
};

export default async function AdminPage({ searchParams }: AdminPageProps) {
  const params = await searchParams;
  const tenant = await resolveTenantSlug(params.tenant);
  const target = new URL(BOARD_ADMIN_BASE_URL);

  target.searchParams.set("tenant", tenant);

  redirect(target.toString());
}
