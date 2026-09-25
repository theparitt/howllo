import { cp, mkdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const root = fileURLToPath(new URL("..", import.meta.url));
const dist = join(root, "dist");
await mkdir(dist, { recursive: true });
for (const name of ["index.html", "assets", "brand"]) {
  await cp(join(root, name), join(dist, name), { recursive: true, force: true });
}
