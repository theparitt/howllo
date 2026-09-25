import { PORTS } from "./ports";

const local = (port: number) => `http://127.0.0.1:${port}`;
const viteEnv = (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env;
const processEnv =
  typeof globalThis !== "undefined" &&
  "process" in globalThis &&
  typeof globalThis.process === "object" &&
  globalThis.process !== null &&
  "env" in globalThis.process &&
  typeof globalThis.process.env === "object" &&
  globalThis.process.env !== null
    ? (globalThis.process.env as Record<string, string | undefined>)
    : undefined;

// Resolved base URLs. Env vars win; otherwise fall back to the canonical local
// port map. No silent fallbacks for caller-supplied routing values elsewhere —
// these are deployment-level defaults only.
export const API_BASE_URL =
  viteEnv?.VITE_API_BASE_URL ??
  processEnv?.HOWLLO_API_BASE_URL ??
  processEnv?.NEXT_PUBLIC_API_BASE_URL ??
  local(PORTS.server);

export const BOARD_ADMIN_BASE_URL =
  viteEnv?.VITE_BOARD_ADMIN_BASE_URL ??
  processEnv?.HOWLLO_BOARD_ADMIN_BASE_URL ??
  processEnv?.NEXT_PUBLIC_BOARD_ADMIN_BASE_URL ??
  local(PORTS.admin);

export const BOARD_WEB_BASE_URL =
  viteEnv?.VITE_BOARD_WEB_BASE_URL ??
  processEnv?.HOWLLO_BOARD_WEB_BASE_URL ??
  processEnv?.NEXT_PUBLIC_BOARD_WEB_BASE_URL ??
  local(PORTS.web);

export const BOARD_WS_URL =
  viteEnv?.VITE_BOARD_WS_URL ??
  processEnv?.HOWLLO_BOARD_WS_URL ??
  processEnv?.NEXT_PUBLIC_BOARD_WS_URL ??
  `ws://127.0.0.1:${PORTS.server}/ws`;

export const DEFAULT_TENANT =
  processEnv?.NEXT_PUBLIC_DEFAULT_TENANT_SLUG ?? "howllo-6soj";

export const ROOIAM_AUTH_BASE =
  processEnv?.VITE_ROOIAM_AUTH_BASE ??
  processEnv?.NEXT_PUBLIC_ROOIAM_AUTH_BASE ??
  "https://api.rooiam.com/v1";

export const ROOIAM_LOGIN_BASE =
  processEnv?.VITE_ROOIAM_LOGIN_BASE ??
  processEnv?.NEXT_PUBLIC_ROOIAM_LOGIN_BASE ??
  "https://www.rooiam.com";

export const ROOIAM_CLIENT_ID =
  processEnv?.VITE_ROOIAM_CLIENT_ID ??
  processEnv?.NEXT_PUBLIC_ROOIAM_CLIENT_ID ??
  "";
