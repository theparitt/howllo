//! WebSocket connection endpoint.
//!
//! Browsers cannot set an `Authorization` header on a WebSocket handshake, so
//! the client passes the Rooiam JWT as a short-lived `?ticket=` query param.
//! We validate it with the same [`RooiamClient`] used for REST auth, resolve
//! the local user, and verify the user is a member of the requested tenant
//! before subscribing the socket to that tenant's event stream.

use actix_web::{get, web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde::Deserialize;

use crate::auth::RooiamClient;
use crate::config::Settings;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::memberships;
use crate::realtime::Hub;

#[derive(Debug, Deserialize)]
pub struct WsConnectQuery {
    /// Rooiam JWT (same token used for REST `Authorization: Bearer`).
    pub ticket: String,
    /// Tenant slug the socket wants to subscribe to.
    pub tenant_slug: String,
}

#[get("/ws")]
pub async fn ws_connect(
    req: HttpRequest,
    body: web::Payload,
    query: web::Query<WsConnectQuery>,
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    hub: web::Data<Hub>,
) -> Result<HttpResponse, AppError> {
    // 1. Authenticate the ticket.
    let client = RooiamClient::new(settings.rooiam_jwt_secret.clone());
    let claims = client
        .validate_token(&query.ticket)
        .map_err(|_| AppError::Unauthorized)?;

    // 2. Resolve the local user (upsert mirrors the REST auth path).
    let user = sqlx::query!(
        r#"
        INSERT INTO users (rooiam_subject, email, display_name)
        VALUES ($1, $2, $3)
        ON CONFLICT (rooiam_subject)
        DO UPDATE SET email = EXCLUDED.email, display_name = EXCLUDED.display_name
        RETURNING id
        "#,
        claims.sub,
        claims
            .email
            .unwrap_or_else(|| "no-email@example.com".to_string()),
        claims.name.unwrap_or_else(|| "Unknown User".to_string()),
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!(error = %e, tenant_slug = query.tenant_slug.as_str(), "ws error resolving user");
        AppError::InternalServerError
    })?;

    // 3. Resolve tenant and verify membership before subscribing. A socket only
    //    ever receives events for a tenant the user actually belongs to.
    let tenant_id = crate::repositories::membership_repository::resolve_tenant_id(
        pool.get_ref(),
        &query.tenant_slug,
    )
    .await?;

    match memberships::check_membership(pool.get_ref(), tenant_id, user.id).await {
        Ok(_role) => {}
        Err(()) => return Err(AppError::Forbidden),
    }

    // 4. Upgrade the connection and subscribe to the tenant channel.
    let (response, mut session, mut msg_stream) =
        actix_ws::handle(&req, body).map_err(|_| AppError::InternalServerError)?;
    let mut events = hub.subscribe(tenant_id);

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                // Outbound: forward tenant events to this socket.
                evt = events.recv() => match evt {
                    Ok(event) => {
                        let payload = serde_json::to_string(&event).unwrap_or_default();
                        if session.text(payload).await.is_err() {
                            break; // client gone
                        }
                    }
                    // We lagged behind. Tell the client to refetch and continue.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let _ = session.text(r#"{"event_type":"resync"}"#).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },

                // Inbound: handle pings and close; ignore client text.
                incoming = msg_stream.next() => match incoming {
                    Some(Ok(actix_ws::Message::Ping(bytes))) => {
                        if session.pong(&bytes).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(actix_ws::Message::Close(reason))) => {
                        let _ = session.close(reason).await;
                        break;
                    }
                    Some(Ok(_)) => {}        // ignore text/binary/pong
                    Some(Err(_)) | None => break,
                },
            }
        }
    });

    Ok(response)
}
