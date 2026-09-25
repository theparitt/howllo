export type PolicyLimits = {
  posts_per_hour: number;
  posts_per_day: number;
  comments_per_hour: number;
  comments_per_day: number;
  board_posts_per_10m: number;
  board_comments_per_10m: number;
  storage_mb: number;
};

export type PlatformPolicy = { defaults: PolicyLimits; caps: PolicyLimits };

export type WorkspacePolicy = {
  effective: PolicyLimits;
  storage_cap_mb: number | null;
  storage_used_bytes: number;
};
