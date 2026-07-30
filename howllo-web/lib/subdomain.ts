// Public boards are served per-workspace on a subdomain (or custom domain):
// `{slug}.<BOARD_BASE_DOMAIN>` → that workspace's board. The bare/app host is the
// tenant dashboard. In dev, `{slug}.localhost` resolves to 127.0.0.1, so it's
// testable without any hosts-file setup.

export const BOARD_BASE_DOMAIN =
  process.env.NEXT_PUBLIC_BOARD_BASE_DOMAIN ?? "localhost";

// Hosts that are the app/dashboard, not a workspace board.
const RESERVED_SUBDOMAINS = new Set(["app", "www", "api", "admin", ""]);

/** The workspace slug encoded in a host's subdomain, or null for the app host. */
export function subdomainSlug(host: string | null | undefined): string | null {
  if (!host) return null;
  const name = host.split(":")[0].toLowerCase().trim();
  if (!name || name === BOARD_BASE_DOMAIN) return null;
  const suffix = `.${BOARD_BASE_DOMAIN}`;
  if (!name.endsWith(suffix)) return null;
  const sub = name.slice(0, name.length - suffix.length);
  if (!sub || sub.includes(".") || RESERVED_SUBDOMAINS.has(sub)) return null;
  return sub;
}
