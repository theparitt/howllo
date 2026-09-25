import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const port = Number(process.env.PORT || 5116);
const types = { ".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8", ".svg": "image/svg+xml" };

createServer(async (request, response) => {
  const path = new URL(request.url || "/", "http://localhost").pathname;
  const file = path === "/" ? "index.html" : path.slice(1);
  const source = file === "assets/howllo-logo-wordmark-horizontal.svg"
    ? resolve(root, "../howllo-landing/brand/howllo-logo-wordmark-horizontal.svg")
    : ["index.html", "styles.css"].includes(file) ? resolve(root, file) : null;
  if (!source) {
    response.writeHead(404).end("Not found");
    return;
  }
  try {
    const body = await readFile(source);
    response.writeHead(200, { "content-type": types[extname(file)] });
    response.end(body);
  } catch {
    response.writeHead(404).end("Not found");
  }
}).listen(port, "0.0.0.0", () => console.log(`Howllo Guides: http://localhost:${port}`));
