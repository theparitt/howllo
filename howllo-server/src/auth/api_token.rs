use actix_web::{web, HttpRequest};
use sha2::{Digest, Sha256};

use crate::db::DbPool;
use crate::errors::AppError;
use crate::repositories::api_token_repository;

#[derive(Debug, Clone)]
pub struct ApiTokenAuth {
    pub tenant_id: uuid::Uuid,
    pub scopes: Vec<String>,
}

pub async fn maybe_api_token(req: &HttpRequest) -> Result<Option<ApiTokenAuth>, AppError> {
    let raw = match bearer_token(req) {
        Some(token) if token.starts_with("howllo_") && !token.starts_with("howllo_ws_") => token,
        _ => return Ok(None),
    };

    let pool = req
        .app_data::<web::Data<DbPool>>()
        .ok_or(AppError::InternalServerError)?;

    let token_hash = hash_token(raw);
    let token = api_token_repository::find_api_token_by_hash(pool.get_ref(), &token_hash)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "error resolving api token");
            AppError::InternalServerError
        })?
        .ok_or(AppError::Unauthorized)?;

    if token.revoked_at.is_some() {
        return Err(AppError::Unauthorized);
    }

    if let Some(expires_at) = token.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(AppError::Unauthorized);
        }
    }

    let _ = api_token_repository::update_last_used_at(pool.get_ref(), &token_hash).await;

    Ok(Some(ApiTokenAuth {
        tenant_id: token.tenant_id,
        scopes: token.scopes,
    }))
}

pub async fn require_optional_api_token_scope(
    req: &HttpRequest,
    scope: &str,
) -> Result<Option<ApiTokenAuth>, AppError> {
    let token = maybe_api_token(req).await?;
    if let Some(token) = &token {
        if !token.scopes.iter().any(|candidate| candidate == scope) {
            return Err(AppError::Forbidden);
        }
    }
    Ok(token)
}

fn bearer_token(req: &HttpRequest) -> Option<&str> {
    req.headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
