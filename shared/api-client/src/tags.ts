import type { Tag } from "@howllo/types";
import type { HowlloClient } from "./client";

export const tags = (client: HowlloClient) => ({
  list: () => client.request<Tag[]>(`/api/tags`),
});
