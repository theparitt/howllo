"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { createComment, followPost, getPostFollowState, unfollowPost, unvotePost, votePost } from "@/lib/api";
import { readStoredBearerToken, subscribeToBearerTokenChange } from "@/components/dev-auth-panel";
import { requestLogin } from "@/components/auth-login";

type PostActionsProps = {
  tenantSlug: string;
  postId: string;
  allowVotes?: boolean;
  voteLabel?: string;
  initiallyVoted?: boolean;
  initiallyFollowing?: boolean;
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

export function PostActions({ tenantSlug, postId, allowVotes = true, voteLabel = "Vote", initiallyVoted = false, initiallyFollowing = false }: PostActionsProps) {
  const router = useRouter();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"vote" | "follow" | null>(null);
  const [voted, setVoted] = useState(initiallyVoted);
  const [following, setFollowing] = useState(initiallyFollowing);

  useEffect(() => setVoted(initiallyVoted), [initiallyVoted]);
  useEffect(() => setFollowing(initiallyFollowing), [initiallyFollowing]);

  useEffect(() => {
    let active = true;
    const sync = () => {
      const token = readStoredBearerToken(tenantSlug).trim();
      if (!token) {
        setFollowing(false);
        return;
      }
      getPostFollowState(postId, token)
        .then((state) => { if (active) setFollowing(state.is_following); })
        .catch(() => { /* Keep the last known state if the request fails. */ });
    };
    sync();
    const unsubscribe = subscribeToBearerTokenChange(() => { sync(); router.refresh(); });
    return () => { active = false; unsubscribe(); };
  }, [postId, tenantSlug, router]);

  async function withToken(
    action: "vote" | "follow",
    run: (token: string) => Promise<void>,
  ) {
    const token = readStoredBearerToken(tenantSlug);
    if (!token) {
      setError("Sign in to take part in this conversation.");
      requestLogin();
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
          aria-pressed={voted}
          onClick={() => withToken("vote", async (token) => {
            if (voted) await unvotePost(postId, token);
            else await votePost(postId, token);
            setVoted(!voted);
          })}
          type="button"
        >
          {busy === "vote" ? "Saving…" : voted ? (voteLabel === "Vote" ? "Voted" : voteLabel === "Like" ? "Liked" : voteLabel === "Helpful" ? "Marked helpful" : "I'm affected ✓") : voteLabel}
        </button> : null}
        <button
          className="ghost-button"
          disabled={busy !== null}
          aria-pressed={following}
          onClick={() => withToken("follow", async (token) => {
            if (following) await unfollowPost(postId, token);
            else await followPost(postId, token);
            setFollowing(!following);
          })}
          type="button"
        >
          {busy === "follow" ? "Saving…" : following ? "Unfollow" : "Follow"}
        </button>
      {error ? <span className="post-engagement__error" role="alert">{error}</span> : null}
    </div>
  );
}

export function CommentComposer({ tenantSlug, postId, isLocked }: PostActionsProps & { isLocked: boolean }) {
  const router = useRouter();
  const draftKey = `howllo.comment-draft.${tenantSlug}.${postId}`;
  const [loadedDraftKey, setLoadedDraftKey] = useState("");
  const [comment, setComment] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setComment(window.sessionStorage.getItem(draftKey) ?? "");
    setLoadedDraftKey(draftKey);
  }, [draftKey]);

  useEffect(() => {
    if (loadedDraftKey !== draftKey) return;
    window.sessionStorage.setItem(draftKey, comment);
  }, [draftKey, loadedDraftKey, comment]);

  if (isLocked) return null;

  return (
      <form
        className="comment-composer"
        onSubmit={(event) => {
          event.preventDefault();
          if (!comment.trim()) return;
          const token = readStoredBearerToken(tenantSlug);
          if (!token) { setError("Sign in to comment. Your text will stay here."); requestLogin(); return; }
          setBusy(true);
          setError(null);
          createComment({ postId, body: comment.trim(), token })
            .then(() => { window.sessionStorage.removeItem(draftKey); setComment(""); router.refresh(); })
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
