export const API_BASE_URL =
  process.env.HOWLLO_API_BASE_URL ??
  process.env.NEXT_PUBLIC_API_BASE_URL ??
  "http://127.0.0.1:5110";

export const BOARD_ADMIN_BASE_URL =
  process.env.HOWLLO_BOARD_ADMIN_BASE_URL ??
  process.env.NEXT_PUBLIC_BOARD_ADMIN_BASE_URL ??
  "http://127.0.0.1:5111";

export const BOARD_WS_URL =
  process.env.HOWLLO_BOARD_WS_URL ??
  process.env.NEXT_PUBLIC_BOARD_WS_URL ??
  "ws://127.0.0.1:5113";

export const DEFAULT_TENANT =
  process.env.NEXT_PUBLIC_DEFAULT_TENANT_SLUG ?? "howllo-6soj";

export const ROOIAM_WIDGET_BASE_URL =
  process.env.NEXT_PUBLIC_ROOIAM_WIDGET_BASE_URL ??
  "https://api.rooiam.com/login-widget";

export const ROOIAM_WIDGET_WORKSPACE_ID =
  process.env.NEXT_PUBLIC_ROOIAM_WIDGET_WORKSPACE_ID ??
  "59d94bdf-2549-4fb8-b57c-900a6a67f3dd";

export const ROOIAM_WIDGET_CLIENT_ID =
  process.env.NEXT_PUBLIC_ROOIAM_WIDGET_CLIENT_ID ?? "";
