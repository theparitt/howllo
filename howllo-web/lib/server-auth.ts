import { cookies } from "next/headers";
import { getWorkspaceAuthCookieName } from "@/lib/auth-cookie";

export async function getServerBearerToken(tenantSlug: string) {
  const cookieStore = await cookies();
  return cookieStore.get(getWorkspaceAuthCookieName(tenantSlug))?.value ?? undefined;
}
