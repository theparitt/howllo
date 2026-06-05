use actix_web::{get, patch, post, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::services::api_token_service;

#[derive(Debug, Deserialize)]
pub struct CreateApiTokenRequest {
    pub tenant_slug: String,
    pub name: String,
    pub scopes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ApiTokenListQuery {
    pub tenant_slug: String,
}

#[get("/api/admin/api-tokens")]
pub async fn list_api_tokens(
    pool: web::Data<DbPool>,
    query: web::Query<ApiTokenListQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let items =
        api_token_service::list_api_tokens(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(items))
}

#[post("/api/admin/api-tokens")]
pub async fn create_api_token(
    pool: web::Data<DbPool>,
    body: web::Json<CreateApiTokenRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".to_string()));
    }
    let created = api_token_service::create_api_token(
        pool.get_ref(),
        &body.tenant_slug,
        body.name.trim(),
        body.scopes.as_deref(),
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Created().json(created))
}

#[patch("/api/admin/api-tokens/{token_id}/revoke")]
pub async fn revoke_api_token(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let token_id = path.into_inner();
    api_token_service::revoke_api_token(pool.get_ref(), token_id, auth.0.id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[cfg(test)]
mod tests;
