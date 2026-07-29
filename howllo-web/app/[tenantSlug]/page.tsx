import { redirect } from "next/navigation";
import { getServerBearerToken } from "@/lib/server-auth";

type TenantRootPageProps = {
  params: Promise<{
    tenantSlug: string;
  }>;
};

export default async function TenantRootPage({ params }: TenantRootPageProps) {
  const { tenantSlug } = await params;
  const signedIn = Boolean(await getServerBearerToken(tenantSlug));
  const slug = encodeURIComponent(tenantSlug);
  redirect(signedIn ? `/${slug}/feed` : `/${slug}/dashboard`);
}
