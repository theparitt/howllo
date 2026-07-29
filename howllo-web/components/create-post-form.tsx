"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useRef, useState } from "react";
import { createPost, uploadImage } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import { buildTenantPath } from "@/lib/default-tenant";

type CreatePostFormProps = {
  tenantSlug: string;
  boardSlug: string;
};

export function CreatePostForm({ tenantSlug, boardSlug }: CreatePostFormProps) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  async function onPickFiles(event: React.ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length === 0) return;
    const token = readStoredBearerToken(tenantSlug);
    if (!token) {
      setError("Sign in before attaching screenshots.");
      return;
    }
    setUploading(true);
    setError(null);
    try {
      const urls = await Promise.all(files.map((file) => uploadImage(file, token)));
      setAttachments((current) => [...current, ...urls]);
    } catch (uploadError) {
      setError(
        uploadError instanceof Error ? uploadError.message : "Upload failed.",
      );
    } finally {
      setUploading(false);
    }
  }

  function removeAttachment(url: string) {
    setAttachments((current) => current.filter((item) => item !== url));
  }

  async function onSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const token = readStoredBearerToken(tenantSlug);

    if (!token) {
      setError("Sign in to this workspace before creating a post.");
      return;
    }

    setPending(true);
    setError(null);
    try {
      const created = await createPost({
        tenantSlug,
        boardSlug,
        title,
        body,
        attachments,
        token,
      });
      const nextSearch = new URLSearchParams(searchParams.toString());
      nextSearch.delete("tenant");
      router.push(buildTenantPath(`/posts/${created.id}`, tenantSlug, tenantSlug, nextSearch));
    } catch (submissionError) {
      setError(
        submissionError instanceof Error
          ? submissionError.message
          : "Failed to create post.",
      );
    } finally {
      setPending(false);
    }
  }

  return (
    <form className="panel field-grid" onSubmit={onSubmit}>
      <div>
        <div className="muted" style={{ marginBottom: "0.45rem", fontSize: "0.86rem" }}>
          Title
        </div>
        <input
          className="field"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          placeholder="What should the team know?"
          required
        />
      </div>
      <div>
        <div className="muted" style={{ marginBottom: "0.45rem", fontSize: "0.86rem" }}>
          Description
        </div>
        <textarea
          className="textarea"
          value={body}
          onChange={(event) => setBody(event.target.value)}
          placeholder="Explain the request, the problem, or the outcome you want."
          required
        />
      </div>
      <div>
        <div className="muted" style={{ marginBottom: "0.45rem", fontSize: "0.86rem" }}>
          Screenshots (optional)
        </div>
        {attachments.length > 0 ? (
          <div className="attachment-grid" style={{ marginBottom: "0.85rem" }}>
            {attachments.map((url) => (
              <div className="attachment-thumb" key={url}>
                <img src={url} alt="attachment" />
                <button
                  type="button"
                  className="attachment-remove"
                  aria-label="Remove"
                  onClick={() => removeAttachment(url)}
                >
                  ×
                </button>
              </div>
            ))}
          </div>
        ) : null}
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          multiple
          hidden
          onChange={onPickFiles}
        />
        <div className="form-actions">
          <div className="form-actions__aside">
            <button
              type="button"
              className="ghost-button"
              disabled={uploading}
              onClick={() => fileInputRef.current?.click()}
            >
              {uploading ? "Uploading…" : "Add screenshot"}
            </button>
          </div>
          <div className="form-actions__primary">
            <button className="button" disabled={pending || uploading} type="submit">
              {pending ? "Creating..." : "Create post"}
            </button>
          </div>
        </div>
      </div>
      {error ? <div className="notice notice--error">{error}</div> : null}
    </form>
  );
}
