import type { Comment } from "@howllo/types";
import type { HowlloClient } from "./client";

export const comments = (client: HowlloClient) => ({
  listByPost: (postId: string) =>
    client.request<Comment[]>(`/api/posts/${postId}/comments`),
});
