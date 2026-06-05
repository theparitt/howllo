// Board identity is hierarchical: tenant.slug + board.slug is unique.
// `name` is the human display name; `slug` drives public URLs; `id` is internal.
export type BoardType =
  | "feature-requests"
  | "bug-reports"
  | "discussions"
  | "announcements"
  | "changelog";

export type Board = {
  id: string;
  slug: string;
  name: string;
  description: string | null;
  board_type: string;
  icon_url: string | null;
};

export type BoardDetail = Board & {
  is_private: boolean;
};

export type BoardSummary = {
  board_id: string;
  board_slug: string;
  board_name: string;
  total_posts: number;
  posts_by_status: Record<string, number>;
  total_votes: number;
  total_comments: number;
};
