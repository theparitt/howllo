"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useRef, useState } from "react";
import Link from "next/link";
import { createPost, uploadImage } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import { buildTenantPath } from "@/lib/default-tenant";

type CreatePostFormProps = {
  tenantSlug: string;
  boardSlug: string;
};

function submissionMessage(error: unknown): string {
  if (!(error instanceof Error)) return "Failed to create post.";
  try {
    const response = JSON.parse(error.message) as { error?: { message?: string } };
    if (response.error?.message) return response.error.message;
  } catch { /* Plain-text error. */ }
  return error.message;
}

export function CreatePostForm({ tenantSlug, boardSlug }: CreatePostFormProps) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [submittedForReview, setSubmittedForReview] = useState(false);
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
      if (created.review_state === "pending") {
        setSubmittedForReview(true);
        return;
      }
      const nextSearch = new URLSearchParams(searchParams.toString());
      nextSearch.delete("tenant");
      router.push(buildTenantPath(`/posts/${created.id}`, tenantSlug, tenantSlug, nextSearch));
    } catch (submissionError) {
      setError(submissionMessage(submissionError));
    } finally {
      setPending(false);
    }
  }

  if (submittedForReview) return <section className="panel empty-state" role="status">
    <h2 className="empty-state__title">Post sent for review</h2>
    <p className="empty-state__copy">It will appear on the board after a moderator approves it. We’ll notify you of the decision.</p>
    <Link className="button" href={buildTenantPath(`/boards/${boardSlug}`, tenantSlug, tenantSlug)}>Back to board</Link>
  </section>;

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
