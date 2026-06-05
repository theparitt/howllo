import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

// Admin runs on 5111. It is an internal tool and does not need SEO.
//
// The shared packages export raw TS from `shared/*/src`, so we alias the
// `@howllo/*` specifiers straight at those sources. This works without a build
// step in each package and keeps types live across the monorepo.
const sharedRoot = (pkg: string, file = "src") =>
  fileURLToPath(new URL(`../shared/${pkg}/${file}`, import.meta.url));

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      "@howllo/api-client": sharedRoot("api-client"),
      "@howllo/types": sharedRoot("types"),
      "@howllo/config": sharedRoot("config"),
      "@howllo/ui": sharedRoot("ui"),
    },
  },
  server: { port: 5111 },
  preview: { port: 5111 },
});
