import { BOARD_WS_URL } from "@/lib/config";

export function buildBoardWebSocketUrl(input: {
  tenantSlug: string;
  boardSlug: string;
  token?: string;
}) {
  const url = new URL(BOARD_WS_URL);
  url.searchParams.set("tenant_slug", input.tenantSlug);
  url.searchParams.set("board_slug", input.boardSlug);

  if (input.token) {
    url.searchParams.set("token", input.token);
  }

  return url.toString();
}
