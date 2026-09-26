export type BoardKind = "feature-requests" | "bug-reports" | "discussions" | "announcements";

export const BOARD_PRESETS: { value: BoardKind; label: string; description: string; defaultVotes: boolean; defaultComments: boolean }[] = [
  { value: "feature-requests", label: "Feature requests", description: "Collect ideas, votes, and progress updates.", defaultVotes: true, defaultComments: true },
  { value: "bug-reports", label: "Bug reports", description: "Collect reproducible reports and track fixes.", defaultVotes: false, defaultComments: true },
  { value: "discussions", label: "Discussions", description: "Start conversations and gather answers.", defaultVotes: false, defaultComments: true },
  { value: "announcements", label: "Announcements", description: "Publish updates from your team. Visitors can read and respond.", defaultVotes: false, defaultComments: true },
];

export function boardKind(type: string): BoardKind {
  if (type === "bug-reports" || type === "support") return "bug-reports";
  if (type === "discussions" || type === "general" || type === "internal") return "discussions";
  if (type === "announcements" || type === "changelog" || type === "updates") return "announcements";
  return "feature-requests";
}

export function boardPreset(type: string) {
  return BOARD_PRESETS.find((preset) => preset.value === boardKind(type))!;
}

export function boardVoteLabel(type: string): string {
  const kind = boardKind(type);
  if (kind === "bug-reports") return "I'm affected";
  if (kind === "discussions") return "Like";
  if (kind === "announcements") return "Helpful";
  return "Vote";
}
