import type { BoardPresentation } from "@/lib/api";

export type BoardCapability =
  | "post.editor" | "post.body"
  | "attachment.upload.pdf" | "attachment.preview.pdf"
  | "attachment.upload.glb" | "attachment.preview.glb";

type BuiltIn = { version: string; slot: string; stylesheet: string; capabilities: readonly BoardCapability[] };

// The public API selects installed packages; it never supplies executable code.
// Adding a component plugin requires a reviewed source change on both sides.
const COMPONENTS: Record<string, BuiltIn> = {
  "simple-markdown": { version: "1.0.0", slot: "board.post.markdown", stylesheet: "/plugins/simple-markdown.css", capabilities: ["post.editor", "post.body"] },
  "pdf-preview": { version: "1.0.0", slot: "board.attachments.pdf", stylesheet: "/plugins/pdf-preview.css", capabilities: ["attachment.upload.pdf", "attachment.preview.pdf"] },
  "model-preview": { version: "1.0.0", slot: "board.attachments.model", stylesheet: "/plugins/model-preview.css", capabilities: ["attachment.upload.glb", "attachment.preview.glb"] },
};

export type ActiveBoardPlugins = { ids: readonly string[]; capabilities: readonly BoardCapability[] };

export function activeBoardPlugins(presentation: Pick<BoardPresentation, "plugins">): ActiveBoardPlugins {
  const ids: string[] = [];
  const capabilities = new Set<BoardCapability>();
  for (const plugin of presentation.plugins ?? []) {
    if (!Object.prototype.hasOwnProperty.call(COMPONENTS, plugin.id)) continue;
    const installed = COMPONENTS[plugin.id];
    if (!installed || plugin.version !== installed.version || plugin.slot !== installed.slot ||
      plugin.stylesheet_path !== installed.stylesheet || plugin.runtime_kind !== "component" ||
      !Array.isArray(plugin.capabilities) ||
      plugin.capabilities.length !== installed.capabilities.length ||
      !installed.capabilities.every((item) => plugin.capabilities?.includes(item))) continue;
    ids.push(plugin.id);
    installed.capabilities.forEach((item) => capabilities.add(item));
  }
  return { ids, capabilities: [...capabilities] };
}

export function hasCapability(plugins: ActiveBoardPlugins, capability: BoardCapability): boolean {
  return plugins.capabilities.includes(capability);
}

export function attachmentKind(url: string): "image" | "pdf" | "glb" | "unknown" {
  let path: string;
  try { path = new URL(url, "https://howllo.invalid").pathname.toLowerCase(); }
  catch { return "unknown"; }
  if (/\.(png|jpe?g|gif|webp)$/.test(path)) return "image";
  if (path.endsWith(".pdf")) return "pdf";
  if (path.endsWith(".glb")) return "glb";
  return "unknown";
}

export function safeAttachmentUrl(url: string): string | null {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" || parsed.protocol === "http:" ? parsed.href : null;
  } catch { return null; }
}

export function attachmentAccept(plugins: ActiveBoardPlugins): string {
  const types = ["image/png", "image/jpeg", "image/gif", "image/webp"];
  if (hasCapability(plugins, "attachment.upload.pdf")) types.push("application/pdf", ".pdf");
  if (hasCapability(plugins, "attachment.upload.glb")) types.push("model/gltf-binary", ".glb");
  return types.join(",");
}
