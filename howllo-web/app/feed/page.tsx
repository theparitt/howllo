import { ActivityFeed } from "@/components/activity-feed";

// The signed-in home feed. Tenant is resolved client-side (from the URL/token)
// inside ActivityFeed, so this page stays a thin shell within the tenant shell.
export default function FeedPage() {
  return <ActivityFeed />;
}
