"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useRef, useState } from "react";
import Link from "next/link";
import { createPost, uploadImage } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";
import { buildTenantPath } from "@/lib/default-tenant";
import type { BoardKind } from "@/lib/board-experience";

type CreatePostFormProps = {
  tenantSlug: string;
  boardSlug: string;
  boardKind: BoardKind;
};

function submissionMessage(error: unknown): string {
  if (!(error instanceof Error)) return "Failed to create post.";
  try {
    const response = JSON.parse(error.message) as { error?: { message?: string } };
    if (response.error?.message) return response.error.message;
  } catch { /* Plain-text error. */ }
  return error.message;
}

export function CreatePostForm({ tenantSlug, boardSlug, boardKind }: CreatePostFormProps) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [steps, setSteps] = useState("");
  const [expected, setExpected] = useState("");
  const [actual, setActual] = useState("");
  const [environment, setEnvironment] = useState("");
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
      const urls = await Promise.all(files.map((file) => uploadImage(file, token, tenantSlug)));
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
        body: boardKind === "bug-reports" ? [
          `Steps to reproduce\n${steps.trim()}`,
          `Expected result\n${expected.trim()}`,
          `Actual result\n${actual.trim()}`,
          environment.trim() ? `Environment\n${environment.trim()}` : "",
        ].filter(Boolean).join("\n\n") : body,
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
          {boardKind === "bug-reports" ? "Short summary" : boardKind === "discussions" ? "Topic or question" : "Feature idea"}
        </div>
        <input
          className="field"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          placeholder={boardKind === "bug-reports" ? "What is broken?" : boardKind === "discussions" ? "What would you like to discuss?" : "What would you like to see?"}
          required
        />
      </div>
      {boardKind === "bug-reports" ? <div className="field-grid">
        <label className="manage-label">Steps to reproduce<textarea className="textarea" value={steps} onChange={(event) => setSteps(event.target.value)} placeholder="1. Open…\n2. Click…" required /></label>
        <label className="manage-label">Expected result<textarea className="textarea" value={expected} onChange={(event) => setExpected(event.target.value)} placeholder="What should have happened?" required /></label>
        <label className="manage-label">Actual result<textarea className="textarea" value={actual} onChange={(event) => setActual(event.target.value)} placeholder="What happened instead?" required /></label>
        <label className="manage-label">Device or browser (optional)<input className="field" value={environment} onChange={(event) => setEnvironment(event.target.value)} placeholder="e.g. Chrome on Windows" /></label>
      </div> : <div>
        <div className="muted" style={{ marginBottom: "0.45rem", fontSize: "0.86rem" }}>
          {boardKind === "discussions" ? "Your message" : "Why would this help?"}
        </div>
        <textarea
          className="textarea"
          value={body}
          onChange={(event) => setBody(event.target.value)}
          placeholder={boardKind === "discussions" ? "Share context so others can join the conversation." : "Describe the problem and the outcome you want."}
          required
        />
      </div>}
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
              {pending ? "Posting…" : boardKind === "bug-reports" ? "Submit bug report" : boardKind === "discussions" ? "Start discussion" : "Submit idea"}
            </button>
          </div>
        </div>
      </div>
      {error ? <div className="notice notice--error">{error}</div> : null}
    </form>
  );
}
