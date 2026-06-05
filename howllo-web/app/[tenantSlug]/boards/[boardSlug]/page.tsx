import BoardPage from "@/app/boards/[boardSlug]/page";

type TenantBoardPageProps = {
  params: Promise<{
    tenantSlug: string;
    boardSlug: string;
  }>;
  searchParams: Promise<{
    sort?: string;
    status?: string;
    page?: string;
  }>;
};

export default async function TenantBoardPage({
  params,
  searchParams,
}: TenantBoardPageProps) {
  const { tenantSlug, boardSlug } = await params;
  const query = await searchParams;

  return BoardPage({
    params: Promise.resolve({ boardSlug }),
    searchParams: Promise.resolve({
      tenant: tenantSlug,
      sort: query.sort,
      status: query.status,
      page: query.page,
    }),
  });
}
