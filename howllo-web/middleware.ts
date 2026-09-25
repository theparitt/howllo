import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";
import { subdomainSlug } from "@/lib/subdomain";

// On a workspace subdomain (`{slug}.<base>`), serve that workspace's public board
// while keeping the clean URL: internally rewrite `/…` → `/{slug}/…` so the
// [tenantSlug] routes render, but the browser still shows `{slug}.<base>/…`.
// The bare/app host is untouched.
export function middleware(req: NextRequest) {
  const slug = subdomainSlug(req.headers.get("host"));
  if (!slug) return NextResponse.next();

  const url = req.nextUrl.clone();
  const path = url.pathname;

  // Already tenant-prefixed, or an asset/api route — leave it alone.
  if (
    path === `/${slug}` ||
    path.startsWith(`/${slug}/`) ||
    path.startsWith("/_next") ||
    path.startsWith("/api") ||
    path.startsWith("/brand") ||
    path.startsWith("/auth")
  ) {
    return NextResponse.next();
  }

  // Root opens the board list; each board opens its posts.
  url.pathname = path === "/" ? `/${slug}` : `/${slug}${path}`;
  return NextResponse.rewrite(url);
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
