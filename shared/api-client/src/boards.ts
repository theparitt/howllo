import type { Board, BoardDetail } from "@howllo/types";
import type { HowlloClient } from "./client";

export const boards = (client: HowlloClient) => ({
  list: () => client.request<Board[]>(`/api/boards`),
  get: (boardSlug: string) =>
    client.request<BoardDetail>(`/api/boards/${boardSlug}`),
});
