import { cookies } from "next/headers";
import { DEV_AUTH_COOKIE } from "@/lib/auth-cookie";

export async function getServerBearerToken() {
  const cookieStore = await cookies();
  return cookieStore.get(DEV_AUTH_COOKIE)?.value ?? undefined;
}
