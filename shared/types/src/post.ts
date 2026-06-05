import type { PostStatus } from "./status";

export type PostListItem = {
  id: string;
  title: string;
  status: PostStatus | string;
  vote_count: number;
  comment_count: number;
  duplicate_of_post_id: string | null;
  created_at: string;
  /** Moderation flags. Only meaningful when listed with include_hidden. */
  is_hidden: boolean;
  deleted_at: string | null;
};

export type PostDetail = {
  id: string;
  title: string;
  body: string;
  status: PostStatus | string;
  vote_count: number;
  is_locked: boolean;
  duplicate_of_post_id: string | null;
  created_at: string;
  board_slug: string;
};

export type PostCreated = {
  id: string;
};

export type RoadmapItem = PostListItem;
