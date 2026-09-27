import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { type Browser, type BrowserContext } from "@playwright/test";
import { missingCapability } from "./environment";

export type Actor = "A-OWNER" | "A-MOD" | "U-FEATURE" | "U-FOREIGN";

const stateDir = process.env.HOWLLO_TEST_PERSONAS_DIR
  ? resolve(process.env.HOWLLO_TEST_PERSONAS_DIR)
  : "";
type StorageState = Exclude<NonNullable<Parameters<Browser["newContext"]>[0]>["storageState"], string | undefined>;

function isLoopback(host: string): boolean {
  return ["localhost", "127.0.0.1", "::1", "[::1]"].includes(host.replace(/^\./, ""));
}

function actorStatePath(actor: Actor): string {
  if (!stateDir) return missingCapability("HOWLLO_TEST_PERSONAS_DIR is not configured");
  const path = resolve(stateDir, `${actor}.storage.json`);
  if (!existsSync(path)) return missingCapability(`${actor} has no private browser storage state`);
  return path;
}

function localState(path: string): StorageState {
  const state = JSON.parse(readFileSync(path, "utf8")) as StorageState;
  if (!Array.isArray(state.cookies) || !Array.isArray(state.origins)) {
    throw new Error("Persona storage state is not a Playwright storageState snapshot");
  }
  // RooIAM sign-in may leave third-party provider cookies in the snapshot.
  // They are never needed by local Howllo tests and are not loaded into contexts.
  return {
    cookies: state.cookies.filter((cookie) => isLoopback(cookie.domain)),
    origins: state.origins.filter((origin) => {
      const url = new URL(origin.origin);
      return url.protocol === "http:" && isLoopback(url.hostname);
    }),
  };
}

function credentialValues(path: string): Set<string> {
  const state = localState(path);
  const values = new Set<string>();
  for (const cookie of state.cookies) {
    if (/howllo.*(auth|session|token)/i.test(cookie.name) && cookie.value) values.add(cookie.value);
  }
  for (const origin of state.origins) {
    for (const entry of origin.localStorage) {
      if (/howllo.*(auth|account|token)/i.test(entry.name) && entry.value) values.add(entry.value);
    }
  }
  if (!values.size) throw new Error("Persona storage state has no Howllo credential");
  return values;
}

export function verifyDistinctActorStates(actors: Actor[]): void {
  const used = new Map<string, Actor>();
  for (const actor of actors) {
    const path = actorStatePath(actor);
    for (const value of credentialValues(path)) {
      const owner = used.get(value);
      if (owner && owner !== actor) throw new Error(`${actor} and ${owner} share a credential; supply separate accounts`);
      used.set(value, actor);
    }
  }
}

export async function newActorContext(browser: Browser, actor: Actor): Promise<BrowserContext> {
  const path = actorStatePath(actor);
  credentialValues(path);
  return browser.newContext({ storageState: localState(path) });
}
