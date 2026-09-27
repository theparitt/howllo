import { test } from "@playwright/test";

function loopbackUrl(name: string, fallback: string): string {
  const raw = process.env[name] || fallback;
  const url = new URL(raw);
  if (url.protocol !== "http:" || !["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)
    || url.username || url.password || url.pathname !== "/" || url.search || url.hash) {
    throw new Error(`${name} must be an HTTP loopback origin. Browser tests never target a public server.`);
  }
  return url.origin;
}

export const local = {
  api: loopbackUrl("HOWLLO_TEST_API_URL", "http://127.0.0.1:7700"),
  admin: loopbackUrl("HOWLLO_TEST_ADMIN_URL", "http://127.0.0.1:7701"),
  app: loopbackUrl("HOWLLO_TEST_APP_URL", "http://127.0.0.1:7702"),
  web: loopbackUrl("HOWLLO_TEST_WEB_URL", "http://127.0.0.1:7703"),
};

export const workspaceA = process.env.HOWLLO_TEST_WORKSPACE_A?.trim() || "";
export const workspaceB = process.env.HOWLLO_TEST_WORKSPACE_B?.trim() || "";
export const privateBoardA = process.env.HOWLLO_TEST_PRIVATE_BOARD_A?.trim() || "";

export function missingCapability(reason: string): never {
  test.info().annotations.push({ type: "MISSING_CAPABILITY", description: reason });
  test.skip(true, `MISSING_CAPABILITY: ${reason}`);
  throw new Error("unreachable after test.skip");
}

export function requireValue(value: string, name: string): string {
  if (!value) return missingCapability(`${name} is not configured`);
  return value;
}

export async function requireLocalService(name: string, origin: string): Promise<void> {
  try {
    const response = await fetch(origin, { method: "GET", redirect: "manual", signal: AbortSignal.timeout(3500) });
    if (response.status >= 500) throw new Error(`HTTP ${response.status}`);
  } catch (error) {
    if (error instanceof Error && /^HTTP /.test(error.message)) throw error;
    missingCapability(`${name} is not running at its local origin`);
  }
}
