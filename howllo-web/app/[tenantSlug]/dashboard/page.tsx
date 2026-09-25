import { redirect } from "next/navigation";

type TenantDashboardPageProps = {
  params: Promise<{
    tenantSlug: string;
  }>;
  searchParams: Promise<{
    board?: string;
  }>;
};

export default async function TenantDashboardPage({
  params,
  searchParams,
}: TenantDashboardPageProps) {
  const { tenantSlug } = await params;
  const query = await searchParams;

  const home = `/${encodeURIComponent(tenantSlug)}`;
  redirect(query.board ? `${home}/boards/${encodeURIComponent(query.board)}` : home);
}
