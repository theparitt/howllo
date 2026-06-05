import DashboardPage from "@/app/dashboard/page";

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

  return DashboardPage({
    searchParams: Promise.resolve({
      tenant: tenantSlug,
      board: query.board,
    }),
  });
}
