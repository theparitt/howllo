import RoadmapPage from "@/app/roadmap/page";

type TenantRoadmapPageProps = {
  params: Promise<{
    tenantSlug: string;
  }>;
  searchParams: Promise<{
    status?: string;
    tag?: string;
  }>;
};

export default async function TenantRoadmapPage({
  params,
  searchParams,
}: TenantRoadmapPageProps) {
  const { tenantSlug } = await params;
  const query = await searchParams;

  return RoadmapPage({
    searchParams: Promise.resolve({
      tenant: tenantSlug,
      status: query.status,
      tag: query.tag,
    }),
  });
}
