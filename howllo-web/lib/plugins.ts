import { API_BASE_URL } from "@/lib/config";

export type PublicPlugin = {
  id: string;
  version: string;
  slot: "workspace.typography" | "board.directory.layout";
  stylesheet_path: string;
};

export type PluginContext = {
  api_version: 1;
  workspace: { slug: string; name: string; site_name: string; accent_color: string | null; background_color: string | null;
    pages: { boards: boolean; feed: boolean; roadmap: boolean } };
  boards: { slug: string; name: string; description: string | null; board_type: string; header_image_url: string | null; background_image_url: string | null }[];
  plugins: PublicPlugin[];
};

export async function getPluginContext(tenantSlug: string): Promise<PluginContext> {
  const response = await fetch(`${API_BASE_URL}/api/plugins/v1/workspaces/${encodeURIComponent(tenantSlug)}/context`, { cache: "no-store" });
  if (!response.ok) throw new Error("Could not load workspace plugins.");
  return response.json() as Promise<PluginContext>;
}
