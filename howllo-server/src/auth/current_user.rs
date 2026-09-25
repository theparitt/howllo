use crate::auth::identity::{resolve_user, ExternalIdentity};
use crate::auth::local_user::resolve_account_token;
use crate::auth::rooiam::resolve_rooiam_access_token;
use crate::auth::workspace_session::resolve_workspace_session_from_request;
use crate::config::Settings;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::users::User;
use actix_web::dev::Payload;
use actix_web::HttpMessage;
use actix_web::{web, FromRequest, HttpRequest};
use futures::future::LocalBoxFuture;
use uuid::Uuid;

pub struct AuthenticatedUser(pub User);

#[derive(Debug, Clone, Copy)]
struct WorkspaceSessionTenantScope(pub Uuid);

pub fn current_workspace_session_tenant_id(req: &HttpRequest) -> Option<Uuid> {
    req.extensions()
        .get::<WorkspaceSessionTenantScope>()
        .map(|scope| scope.0)
}

async fn authenticate_with_request(req: &HttpRequest) -> Result<Option<User>, AppError> {
    let pool = req
        .app_data::<web::Data<DbPool>>()
        .ok_or(AppError::InternalServerError)?;

    let auth_header = match req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        Some(token) => token,
        None => return Ok(None),
    };

    if let Some(session) = resolve_workspace_session_from_request(req).await? {
        req.extensions_mut()
            .insert(WorkspaceSessionTenantScope(session.tenant_id));
        return Ok(Some(session.user));
    }

    if let Some(user) = resolve_account_token(pool.get_ref(), auth_header).await? {
        return Ok(Some(user));
    }

    if auth_header.starts_with("howllo_") {
        return Ok(None);
    }

    let settings = req
        .app_data::<web::Data<Settings>>()
        .ok_or(AppError::InternalServerError)?;
    if let Some(user) = crate::auth::local_admin::resolve_admin_token(
        pool.get_ref(),
        settings.get_ref(),
        auth_header,
    )
    .await?
    {
        return Ok(Some(user));
    }
    let identity = resolve_rooiam_access_token(settings.get_ref(), auth_header).await?;

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

    Ok(Some(user))
}

pub async fn maybe_authenticated_user(req: &HttpRequest) -> Result<Option<User>, AppError> {
    authenticate_with_request(req).await
}

impl FromRequest for AuthenticatedUser {
    type Error = AppError;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let req = req.clone();

        Box::pin(async move {
            match authenticate_with_request(&req).await? {
                Some(user) => Ok(AuthenticatedUser(user)),
                None => Err(AppError::Unauthorized),
            }
        })
    }
}
