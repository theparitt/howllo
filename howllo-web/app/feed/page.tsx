import { ActivityFeed } from "@/components/activity-feed";
import { ApiError, getTenantBranding } from "@/lib/api";
import { notFound } from "next/navigation";

// The signed-in home feed. Tenant is resolved client-side (from the URL/token)
// inside ActivityFeed, so this page stays a thin shell within the tenant shell.
export default async function FeedPage({ searchParams }: { searchParams?: Promise<{ tenant?: string }> } = {}) {
  const tenant = (await searchParams)?.tenant;
  if (tenant) {
    let branding;
    try {
      branding = await getTenantBranding(tenant);
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) notFound();
      throw error;
    }
    if (!branding.show_feed) notFound();
  }
  return <ActivityFeed />;
}
