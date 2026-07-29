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

    if auth_header.starts_with("howllo_") {
        return Ok(None);
    }

    let settings = req
        .app_data::<web::Data<Settings>>()
        .ok_or(AppError::InternalServerError)?;
    let identity = resolve_rooiam_access_token(settings.get_ref(), auth_header).await?;

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (rooiam_subject, email, display_name)
        VALUES ($1, $2, $3)
        ON CONFLICT (rooiam_subject)
        DO UPDATE SET
            email = EXCLUDED.email,
            updated_at = NOW()
        RETURNING id, rooiam_subject, email, display_name, avatar_url, created_at, updated_at
        "#,
    )
    .bind(identity.sub)
    .bind(
        identity
            .email
            .unwrap_or_else(|| "no-email@example.com".to_string()),
    )
    .bind(identity.name.unwrap_or_else(|| "Unknown User".to_string()))
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "database error resolving authenticated user");
        AppError::InternalServerError
    })?;

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
