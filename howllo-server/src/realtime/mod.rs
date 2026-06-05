//! Realtime fan-out over WebSocket.
//!
//! This module is *additive* and never on the correctness path. The same
//! domain events that persist a notification row and emit a webhook also call
//! [`Hub::broadcast`] to push a lightweight message to connected sockets.
//! Clients treat these messages as a "ping to refetch" — the REST API and the
//! `notifications` table remain the source of truth.
//!
//! ## Scoping
//! Every channel is keyed by `tenant_id`. A socket only ever receives events
//! for the tenant it authenticated and is a member of. This mirrors the
//! tenant-isolation rule enforced on every table and query.
//!
//! ## Single-process today
//! The [`Hub`] uses an in-process `tokio::sync::broadcast` channel per tenant.
//! That is correct for a single server instance. If we ever run multiple
//! instances, put Postgres `LISTEN/NOTIFY` (or Redis pub/sub) behind this same
//! `broadcast` API so call sites do not change.

pub mod handler;

use dashmap::DashMap;
use serde::Serialize;
use tokio::sync::broadcast;
use uuid::Uuid;

/// How many messages a slow client may lag before it is dropped. Bounded on
/// purpose: realtime is best-effort, and a stalled socket must never grow
/// memory without limit. A lagging receiver gets a `Lagged` error and should
/// reconnect + refetch.
const CHANNEL_CAPACITY: usize = 256;

/// A tenant-scoped realtime event. Kept deliberately small — it carries enough
/// for a client to know *what changed* and decide what to refetch, not the full
/// changed entity.
#[derive(Debug, Clone, Serialize)]
pub struct RealtimeEvent {
    /// e.g. `post.status_changed`, `post.created`, `comment.created`,
    /// `comment.official_response`, `post.vote_changed`.
    pub event_type: String,
    /// The board this event belongs to, when applicable. Clients viewing a
    /// single board can ignore events for other boards.
    pub board_id: Option<Uuid>,
    /// The post this event belongs to, when applicable.
    pub post_id: Option<Uuid>,
}

/// Shared realtime hub. Place behind `web::Data` and clone freely; the inner
/// map and senders are reference-counted.
#[derive(Clone, Default)]
pub struct Hub {
    channels: std::sync::Arc<DashMap<Uuid, broadcast::Sender<RealtimeEvent>>>,
}

impl Hub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get (or lazily create) the broadcast sender for a tenant.
    fn sender_for(&self, tenant_id: Uuid) -> broadcast::Sender<RealtimeEvent> {
        self.channels
            .entry(tenant_id)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .clone()
    }

    /// Subscribe a new socket to a tenant's event stream.
    pub fn subscribe(&self, tenant_id: Uuid) -> broadcast::Receiver<RealtimeEvent> {
        self.sender_for(tenant_id).subscribe()
    }

    /// Publish an event to all sockets of a tenant.
    ///
    /// Intentionally infallible from the caller's view: if there are no live
    /// receivers, `send` returns `Err` and we simply drop it. A realtime miss
    /// must never fail the domain write that triggered it.
    pub fn broadcast(&self, tenant_id: Uuid, event: RealtimeEvent) {
        // Only allocate/keep a channel while someone is listening. If no entry
        // exists yet, there are no subscribers, so there is nothing to do.
        if let Some(sender) = self.channels.get(&tenant_id) {
            let _ = sender.send(event);
        }
    }
}
