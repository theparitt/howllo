"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { createComment, followPost, votePost } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";

type PostActionsProps = {
  tenantSlug: string;
  postId: string;
  allowVotes?: boolean;
  voteLabel?: string;
};

function actionErrorMessage(cause: unknown, fallback: string): string {
  if (!(cause instanceof Error)) return fallback;
  try {
    const response = JSON.parse(cause.message) as { error?: { message?: string } };
    return response.error?.message || cause.message;
  } catch {
    return cause.message;
  }
}

export function PostActions({ tenantSlug, postId, allowVotes = true, voteLabel = "Vote" }: PostActionsProps) {
  const router = useRouter();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"vote" | "follow" | null>(null);

  async function withToken(
    action: "vote" | "follow",
    run: (token: string) => Promise<void>,
  ) {
    const token = readStoredBearerToken(tenantSlug);
    if (!token) {
      setError("Sign in to vote or follow this post.");
      return;
    }

    setBusy(action);
    setError(null);
    try {
      await run(token);
      router.refresh();
    } catch (submissionError) {
      setError(actionErrorMessage(submissionError, "Action failed."));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="post-engagement">
        {allowVotes ? <button
          className="button"
          disabled={busy !== null}
          onClick={() => withToken("vote", (token) => votePost(postId, token))}
          type="button"
        >
          {busy === "vote" ? "Saving…" : voteLabel}
        </button> : null}
        <button
          className="ghost-button"
          disabled={busy !== null}
          onClick={() => withToken("follow", (token) => followPost(postId, token))}
          type="button"
        >
          {busy === "follow" ? "Following..." : "Follow"}
        </button>
      {error ? <span className="post-engagement__error" role="alert">{error}</span> : null}
    </div>
  );
}

export function CommentComposer({ tenantSlug, postId, isLocked }: PostActionsProps & { isLocked: boolean }) {
  const router = useRouter();
  const [comment, setComment] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (isLocked) return null;

  return (
      <form
        className="comment-composer"
        onSubmit={(event) => {
          event.preventDefault();
          if (!comment.trim()) return;
          const token = readStoredBearerToken(tenantSlug);
          if (!token) { setError("Sign in to comment."); return; }
          setBusy(true);
          setError(null);
          createComment({ postId, body: comment.trim(), token })
            .then(() => { setComment(""); router.refresh(); })
            .catch((cause) => setError(actionErrorMessage(cause, "Could not post comment.")))
            .finally(() => setBusy(false));
        }}
      >
        <label htmlFor="post-comment">Add a comment</label>
        <textarea
          id="post-comment"
          className="textarea"
          disabled={busy}
          placeholder="Write a comment..."
          value={comment}
          onChange={(event) => setComment(event.target.value)}
        />
        {error ? <div className="notice notice--error" role="alert">{error}</div> : null}
        <div><button className="button" disabled={busy || !comment.trim()} type="submit">
          {busy ? "Posting..." : "Post comment"}
        </button></div>
      </form>
  );
}
