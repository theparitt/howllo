import type {
  PaginatedResponse,
  PostCreated,
  PostDetail,
  PostListItem,
} from "@howllo/types";
import type { HowlloClient } from "./client";

export const posts = (client: HowlloClient) => ({
  /**
   * List a board's posts. `includeHidden` (admin-only, enforced server-side)
   * also returns hidden and soft-deleted posts for moderation review.
   */
  listByBoard: (boardSlug: string, opts: { includeHidden?: boolean } = {}) => {
    const suffix = opts.includeHidden ? `?include_hidden=true` : "";
    return client
      .request<PaginatedResponse<PostListItem>>(
        `/api/boards/${boardSlug}/posts${suffix}`,
      )
      .then((payload) => payload.items);
  },
  create: (
    boardSlug: string,
    input: { title: string; body: string; attachments?: string[] },
  ) =>
    client.request<PostCreated>(`/api/boards/${boardSlug}/posts`, {
      method: "POST",
      body: JSON.stringify({
        tenant_slug: client.tenant,
        title: input.title,
        body: input.body,
        attachments: input.attachments ?? [],
      }),
    }),
  update: (postId: string, input: { title: string; body: string }) =>
    client.request<void>(`/api/posts/${postId}`, {
      method: "PATCH",
      body: JSON.stringify(input),
    }),
  get: (postId: string) =>
    client.request<PostDetail>(`/api/posts/${postId}`),
});
