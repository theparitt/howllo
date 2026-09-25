import { redirect } from "next/navigation";

// Tenant management is served by the standalone howllo-app frontend.
export default async function TenantAppPage({ params }: { params: Promise<{ tenantSlug: string }> }) {
  const { tenantSlug } = await params;
  const appOrigin = process.env.HOWLLO_APP_ORIGIN ?? process.env.NEXT_PUBLIC_HOWLLO_APP_URL ?? "http://localhost:7702";
  redirect(`${appOrigin.replace(/\/$/, "")}/app/${encodeURIComponent(tenantSlug)}`);
}
