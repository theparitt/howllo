// Post status / roadmap state machine. Transitions are admin-controlled and
// enforced server-side; this enum is for display and client-side hints only.
export type PostStatus =
  | "under_review"
  | "planned"
  | "in_progress"
  | "done"
  | "declined";

export type StatusHistoryItem = {
  id: string;
  old_status: string | null;
  new_status: string;
  reason: string | null;
  actor_display_name: string;
  created_at: string;
};
