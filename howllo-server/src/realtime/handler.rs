//! WebSocket connection endpoint.
//!
//! Browsers cannot set an `Authorization` header on a WebSocket handshake, so
//! the client passes a workspace session token in `?ticket=`. Existing RooIAM
//! bearer tokens remain accepted for compatibility. Both paths verify that
//! the user belongs to the requested workspace before subscribing to events.

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
    /// Workspace session token, or a RooIAM bearer token for legacy clients.
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
        let session = resolve_workspace_session_from_token(pool.get_ref(), settings.get_ref(), &query.ticket)
            .await?
            .ok_or(AppError::Unauthorized)?;
        if session.tenant_id != tenant_id {
            return Err(AppError::Forbidden);
        }
        session.user.id
    } else {
        let identity = resolve_rooiam_access_token(settings.get_ref(), &query.ticket).await?;
        if crate::external_identity::linked_workspace(pool.get_ref(), settings.get_ref(), tenant_id).await? {
            let subject_segment = crate::external_identity::encoded_subject(&identity.sub)?;
            let member = crate::external_identity::call_bridge(
                settings.get_ref(), reqwest::Method::GET,
                &format!("/v1/workspaces/{tenant_id}/subjects/{subject_segment}"), None, None,
            ).await?;
            if member.get("subject").and_then(|v| v.as_str()) != Some(identity.sub.as_str())
                || member.get("status").and_then(|v| v.as_str()) != Some("active") {
                return Err(AppError::Forbidden);
            }
        }
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
