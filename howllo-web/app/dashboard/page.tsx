import { redirect } from "next/navigation";
import { getBootstrapTenant } from "@/lib/api";

type Props = {
  searchParams: Promise<{ tenant?: string; board?: string }>;
};

// Keep old dashboard URLs working without presenting a second public home page.
export default async function OldDashboard({ searchParams }: Props) {
  const { tenant, board } = await searchParams;
  const slug = tenant?.trim() || (await getBootstrapTenant()).default_tenant_slug;
  const home = `/${encodeURIComponent(slug)}`;
  redirect(board ? `${home}/boards/${encodeURIComponent(board)}` : home);
}
