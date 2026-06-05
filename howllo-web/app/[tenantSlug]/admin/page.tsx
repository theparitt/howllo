import AdminPage from "@/app/admin/page";

type TenantAdminPageProps = {
  params: Promise<{
    tenantSlug: string;
  }>;
};

export default async function TenantAdminPage({ params }: TenantAdminPageProps) {
  const { tenantSlug } = await params;

  return AdminPage({
    searchParams: Promise.resolve({
      tenant: tenantSlug,
    }),
  });
}
