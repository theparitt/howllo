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

use crate::auth::identity::{resolve_user, ExternalIdentity};
use crate::auth::rooiam::resolve_rooiam_access_token;
use crate::auth::workspace_session::{
    is_workspace_session_token, resolve_workspace_session_from_token,
};
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
    let tenant_id = crate::repositories::membership_repository::resolve_tenant_id(
        pool.get_ref(),
        &query.tenant_slug,
    )
    .await?;

    let user_id = if is_workspace_session_token(&query.ticket) {
        let session = resolve_workspace_session_from_token(pool.get_ref(), &query.ticket)
            .await?
            .ok_or(AppError::Unauthorized)?;
        if session.tenant_id != tenant_id {
            return Err(AppError::Forbidden);
        }
        session.user.id
    } else {
        let identity = resolve_rooiam_access_token(settings.get_ref(), &query.ticket).await?;
        let user = resolve_user(
            pool.get_ref(),
            &ExternalIdentity {
                provider_id: "rooiam".into(),
                subject: identity.sub,
                email: identity.email,
                name: identity.name,
            },
        )
        .await?;
        user.id
    };

    match memberships::check_membership(pool.get_ref(), tenant_id, user_id).await {
        Ok(_role) => {}
        Err(()) => return Err(AppError::Forbidden),
    }

    // 2. Upgrade the connection and subscribe to the tenant channel.
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
