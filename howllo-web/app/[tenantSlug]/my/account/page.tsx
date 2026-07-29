import MyAccountPage from "@/app/my/account/page";

type TenantMyAccountPageProps = {
  params: Promise<{
    tenantSlug: string;
  }>;
};

export default async function TenantMyAccountPage({ params }: TenantMyAccountPageProps) {
  const { tenantSlug } = await params;

  return MyAccountPage({
    searchParams: Promise.resolve({
      tenant: tenantSlug,
    }),
  });
}
