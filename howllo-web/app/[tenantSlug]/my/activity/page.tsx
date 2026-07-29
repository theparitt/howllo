import MyActivityPage from "@/app/my/activity/page";

type TenantMyActivityPageProps = {
  params: Promise<{
    tenantSlug: string;
  }>;
};

export default async function TenantMyActivityPage({ params }: TenantMyActivityPageProps) {
  const { tenantSlug } = await params;

  return MyActivityPage({
    searchParams: Promise.resolve({
      tenant: tenantSlug,
    }),
  });
}
