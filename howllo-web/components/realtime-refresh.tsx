"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { readStoredBearerToken, subscribeToBearerTokenChange } from "@/components/dev-auth-panel";
import { buildBoardWebSocketUrl, type WorkspaceRealtimeEvent } from "@/lib/realtime";

type RealtimeRefreshProps = {
  tenantSlug: string;
  boardId?: string;
  postId?: string;
};

const REFRESH_DEBOUNCE_MS = 350;

function eventMatchesScope(
  event: WorkspaceRealtimeEvent,
  scope: { boardId?: string; postId?: string },
) {
  if (event.event_type === "resync") {
    return true;
  }

  if (scope.postId) {
    return !event.post_id || event.post_id === scope.postId;
  }

  if (scope.boardId) {
    return !event.board_id || event.board_id === scope.boardId;
  }

  return true;
}

export function RealtimeRefresh({ tenantSlug, boardId, postId }: RealtimeRefreshProps) {
  const router = useRouter();

  useEffect(() => {
    let socket: WebSocket | null = null;
    let refreshTimer: number | null = null;
    let reconnectTimer: number | null = null;
    let destroyed = false;

    const requestRefresh = () => {
      if (refreshTimer !== null) {
        return;
      }

      refreshTimer = window.setTimeout(() => {
        refreshTimer = null;
        router.refresh();
      }, REFRESH_DEBOUNCE_MS);
    };

    const cleanupSocket = () => {
      if (socket) {
        socket.onopen = null;
        socket.onmessage = null;
        socket.onerror = null;
        socket.onclose = null;
        socket.close();
        socket = null;
      }
    };

    const connect = () => {
      cleanupSocket();

      const token = readStoredBearerToken(tenantSlug).trim();
      if (!token) {
        return;
      }

      socket = new WebSocket(
        buildBoardWebSocketUrl({
          tenantSlug,
          token,
        }),
      );

      socket.onmessage = (message) => {
        try {
          const event = JSON.parse(message.data) as WorkspaceRealtimeEvent;
          if (eventMatchesScope(event, { boardId, postId })) {
            requestRefresh();
          }
        } catch {
          requestRefresh();
        }
      };

      socket.onclose = () => {
        if (destroyed) {
          return;
        }

        reconnectTimer = window.setTimeout(() => {
          reconnectTimer = null;
          connect();
        }, 1000);
      };

      socket.onerror = () => {
        cleanupSocket();
      };
    };

    connect();
    const unsubscribe = subscribeToBearerTokenChange(() => {
      connect();
    });

    return () => {
      destroyed = true;
      unsubscribe();
      cleanupSocket();
      if (refreshTimer !== null) {
        window.clearTimeout(refreshTimer);
      }
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer);
      }
    };
  }, [boardId, postId, router, tenantSlug]);

  return null;
}
