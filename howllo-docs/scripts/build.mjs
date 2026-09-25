import { cp, mkdir, rm } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const output = resolve(root, "dist");
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
for (const file of ["index.html", "styles.css"]) {
  await cp(resolve(root, file), resolve(output, file));
}
await mkdir(resolve(output, "assets"), { recursive: true });
await cp(
  resolve(root, "../howllo-landing/brand/howllo-logo-wordmark-horizontal.svg"),
  resolve(output, "assets/howllo-logo-wordmark-horizontal.svg"),
);
console.log(`Built Howllo Guides into ${output}`);
