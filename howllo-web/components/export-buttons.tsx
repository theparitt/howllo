"use client";

import { useState } from "react";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import { API_BASE_URL } from "@/lib/config";

type ExportButtonsProps = {
  tenantSlug: string;
};

export function ExportButtons({ tenantSlug }: ExportButtonsProps) {
  const [busy, setBusy] = useState<"json" | "csv" | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function download(kind: "json" | "csv") {
    const token = readStoredBearerToken(tenantSlug);
    if (!token) {
      setError("Sign in to this workspace before using exports.");
      return;
    }

    setBusy(kind);
    setError(null);

    try {
      const response = await fetch(
        `${API_BASE_URL}/api/admin/export/posts.${kind}?tenant_slug=${encodeURIComponent(tenantSlug)}`,
        {
          headers: {
            Authorization: token,
          },
        },
      );

      if (!response.ok) {
        throw new Error(await response.text());
      }

      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `howllo-posts-${tenantSlug}.${kind}`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
    } catch (downloadError) {
      setError(
        downloadError instanceof Error ? downloadError.message : "Export failed.",
      );
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="stack stack--tight">
      <div className="toolbar">
        <button
          className="button"
          disabled={busy !== null}
          onClick={() => download("json")}
          type="button"
        >
          {busy === "json" ? "Exporting JSON..." : "Export JSON"}
        </button>
        <button
          className="ghost-button"
          disabled={busy !== null}
          onClick={() => download("csv")}
          type="button"
        >
          {busy === "csv" ? "Exporting CSV..." : "Export CSV"}
        </button>
      </div>
      {error ? <div className="notice notice--error">{error}</div> : null}
    </div>
  );
}
