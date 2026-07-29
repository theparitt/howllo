import { BOARD_WS_URL } from "@/lib/config";

export type WorkspaceRealtimeEvent = {
  event_type: string;
  board_id?: string | null;
  post_id?: string | null;
};

export function buildBoardWebSocketUrl(input: {
  tenantSlug: string;
  token?: string;
}) {
  const url = new URL(BOARD_WS_URL);
  url.searchParams.set("tenant_slug", input.tenantSlug);

  if (input.token) {
    url.searchParams.set("ticket", input.token.replace(/^Bearer\s+/i, "").trim());
  }

  return url.toString();
}
