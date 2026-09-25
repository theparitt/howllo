import FeedPage from "@/app/feed/page";
import { ApiError, getTenantBranding } from "@/lib/api";
import { notFound } from "next/navigation";

export default async function TenantFeedPage({ params }: { params: Promise<{ tenantSlug: string }> }) {
  const { tenantSlug } = await params;
  let branding;
  try {
    branding = await getTenantBranding(tenantSlug);
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) notFound();
    throw error;
  }
  if (!branding.show_feed) notFound();
  return FeedPage({ searchParams: Promise.resolve({ tenant: tenantSlug }) });
}
