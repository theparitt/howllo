import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const envFile = resolve(root, ".env.production");
if (existsSync(envFile)) process.loadEnvFile(envFile);

const dryRun = process.argv.includes("--dry-run");
const requiredUrls = [
  "NEXT_PUBLIC_API_BASE_URL",
  "NEXT_PUBLIC_HOWLLO_PUBLIC_WEB_URL",
  "NEXT_PUBLIC_HOWLLO_APP_URL",
];
for (const key of requiredUrls) {
  const value = process.env[key];
  if (!value) throw new Error(`${key} is required. See .env.production.example`);
  const url = new URL(value);
  if (url.protocol !== "https:" || !url.hostname.includes(".") || url.hostname.endsWith(".example")) {
    throw new Error(`${key} must be a public HTTPS URL (received ${url.origin})`);
  }
}
const providers = {
  "howllo-app": process.env.HOWLLO_APP_AUTH_PROVIDERS || "local",
  "howllo-web": process.env.HOWLLO_WEB_AUTH_PROVIDERS || "local",
};
for (const [target, selected] of Object.entries(providers)) {
  const values = selected.split(",").map(value => value.trim()).filter(Boolean);
  if (!values.length || values.some(value => !["local", "rooiam", "oidc", "oauth2", "disabled"].includes(value))) {
    throw new Error(`Invalid ${target} auth providers: ${selected}`);
  }
  if (values.includes("rooiam")) {
    for (const key of ["NEXT_PUBLIC_ROOIAM_WIDGET_WORKSPACE_ID", "NEXT_PUBLIC_ROOIAM_WIDGET_CLIENT_ID"]) {
      if (!process.env[key] || process.env[key].startsWith("YOUR_")) {
        throw new Error(`${key} is required when ${target} uses RooIAM`);
      }
    }
  }
}

const env = {
  ...process.env,
  VITE_API_BASE_URL: process.env.NEXT_PUBLIC_API_BASE_URL,
};
const targets = ["howllo-app", "howllo-web", "howllo-admin", "howllo-landing", "howllo-docs"];
for (const target of targets) {
  const targetEnv = {
    ...env,
    NEXT_PUBLIC_HOWLLO_AUTH_PROVIDER: providers[target]?.split(",")[0] || "local",
    NEXT_PUBLIC_HOWLLO_AUTH_PROVIDERS: providers[target] || "local",
  };
  const command = dryRun
    ? target === "howllo-app" || target === "howllo-web"
      ? [["npm", "-w", target, "run", "cf:build"], ["npm", "-w", target, "exec", "--", "wrangler", "deploy", "--dry-run"]]
      : [["npm", "-w", target, "run", "build"], ["npm", "-w", target, "exec", "--", "wrangler", "deploy", "--dry-run"]]
    : [["npm", "-w", target, "run", "deploy"]];
  for (const [binary, ...args] of command) {
    console.log(`\n[${target}] ${binary} ${args.join(" ")}`);
    const result = spawnSync(binary, args, { cwd: root, env: targetEnv, stdio: "inherit" });
    if (result.error) throw result.error;
    if (result.status !== 0) process.exit(result.status ?? 1);
  }
}
