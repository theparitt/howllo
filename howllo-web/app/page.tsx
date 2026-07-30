import { redirect } from "next/navigation";
import { WorkspaceHome } from "@/components/workspace-home";

type HomePageProps = {
  searchParams: Promise<{
    tenant?: string;
  }>;
};

// The app root is the signed-in tenant's home: pick or create a workspace.
// An explicit ?tenant= still jumps straight into that workspace.
export default async function HomePage({ searchParams }: HomePageProps) {
  const { tenant } = await searchParams;
  if (tenant?.trim()) {
    redirect(`/${encodeURIComponent(tenant.trim())}`);
  }
  return <WorkspaceHome />;
}
