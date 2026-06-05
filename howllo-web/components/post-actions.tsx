"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { createComment, followPost, votePost } from "@/lib/api";
import { readStoredBearerToken } from "@/components/dev-auth-panel";

type PostActionsProps = {
  postId: string;
  isLocked: boolean;
};

export function PostActions({ postId, isLocked }: PostActionsProps) {
  const router = useRouter();
  const [comment, setComment] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"vote" | "follow" | "comment" | null>(null);

  async function withToken(
    action: "vote" | "follow" | "comment",
    run: (token: string) => Promise<void>,
  ) {
    const token = readStoredBearerToken();
    if (!token) {
      setError("Save a dev bearer token in the header before using write actions.");
      return;
    }

    setBusy(action);
    setError(null);
    try {
      await run(token);
      if (action === "comment") {
        setComment("");
      }
      router.refresh();
    } catch (submissionError) {
      setError(
        submissionError instanceof Error
          ? submissionError.message
          : "Action failed.",
      );
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="panel">
      <h2 className="section-title" style={{ fontSize: "1.15rem" }}>Participate</h2>
      <div className="toolbar" style={{ marginTop: "1rem" }}>
        <button
          className="button"
          disabled={busy !== null}
          onClick={() => withToken("vote", (token) => votePost(postId, token))}
          type="button"
        >
          {busy === "vote" ? "Voting..." : "Vote"}
        </button>
        <button
          className="ghost-button"
          disabled={busy !== null}
          onClick={() => withToken("follow", (token) => followPost(postId, token))}
          type="button"
        >
          {busy === "follow" ? "Following..." : "Follow"}
        </button>
      </div>
      <hr className="divider" />
      <form
        className="field-grid"
        onSubmit={(event) => {
          event.preventDefault();
          if (isLocked) return;
          withToken("comment", (token) =>
            createComment({
              postId,
              body: comment,
              token,
            }).then(() => undefined),
          );
        }}
      >
        <div className="muted" style={{ fontSize: "0.88rem" }}>
          Add a comment
        </div>
        <textarea
          className="textarea"
          disabled={isLocked || busy !== null}
          placeholder={
            isLocked
              ? "This post is locked."
              : "Share context, use cases, or follow-up questions."
          }
          value={comment}
          onChange={(event) => setComment(event.target.value)}
        />
        {error ? <div className="notice notice--error">{error}</div> : null}
        <button className="button" disabled={isLocked || busy !== null} type="submit">
          {busy === "comment" ? "Posting..." : "Post comment"}
        </button>
      </form>
    </section>
  );
}
