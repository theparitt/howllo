export const API_BASE_URL =
  process.env.HOWLLO_API_BASE_URL ??
  process.env.NEXT_PUBLIC_API_BASE_URL ??
  "http://127.0.0.1:5110";

function deriveWebSocketBaseUrl(httpUrl: string) {
  if (httpUrl.startsWith("https://")) {
    return `wss://${httpUrl.slice("https://".length)}`;
  }

  if (httpUrl.startsWith("http://")) {
    return `ws://${httpUrl.slice("http://".length)}`;
  }

  return httpUrl;
}

export const BOARD_ADMIN_BASE_URL =
  process.env.HOWLLO_BOARD_ADMIN_BASE_URL ??
  process.env.NEXT_PUBLIC_BOARD_ADMIN_BASE_URL ??
  "http://127.0.0.1:5111";

export const BOARD_WS_URL =
  process.env.HOWLLO_BOARD_WS_URL ??
  process.env.NEXT_PUBLIC_BOARD_WS_URL ??
  `${deriveWebSocketBaseUrl(API_BASE_URL)}/ws`;

export const DEFAULT_TENANT =
  process.env.NEXT_PUBLIC_DEFAULT_TENANT_SLUG ?? "howllo-6soj";
