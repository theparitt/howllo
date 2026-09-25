type StatusPillProps = {
  status: string;
};

const LABELS: Record<string, string> = {
  open: "Open",
  under_review: "New",
  "under-review": "New",
  planned: "Planned",
  in_progress: "In progress",
  "in-progress": "In progress",
  done: "Done",
  declined: "Declined",
};

export function StatusPill({ status }: StatusPillProps) {
  const label = LABELS[status] ?? status.replace(/[-_]/g, " ");

  return (
    <span className="chip" data-status={status}>
      {label}
    </span>
  );
}
