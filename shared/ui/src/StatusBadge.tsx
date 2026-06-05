import type { PostStatus } from "@howllo/types";

const LABELS: Record<PostStatus, string> = {
  under_review: "Under review",
  planned: "Planned",
  in_progress: "In progress",
  done: "Done",
  declined: "Declined",
};

export function StatusBadge({ status }: { status: PostStatus | string }) {
  const label = LABELS[status as PostStatus] ?? status;
  return <span className="howllo-status-badge" data-status={status}>{label}</span>;
}
