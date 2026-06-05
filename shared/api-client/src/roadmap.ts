import type { RoadmapItem } from "@howllo/types";
import type { HowlloClient } from "./client";

export const roadmap = (client: HowlloClient) => ({
  list: () => client.request<RoadmapItem[]>(`/api/roadmap`),
  byStatus: () => client.request<RoadmapItem[]>(`/api/roadmap/by-status`),
  byTag: () => client.request<RoadmapItem[]>(`/api/roadmap/by-tag`),
});
