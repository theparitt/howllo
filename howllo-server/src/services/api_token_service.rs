use serde_json::json;
use uuid::Uuid;

use crate::audit::{self, record_in_tx, AuditEntry};
use crate::auth::require_permission;
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::errors::AppError;
use crate::repositories::api_token_repository;

#[derive(Debug, serde::Serialize)]
pub struct ApiTokenCreatedDto {
    pub id: Uuid,
    pub name: String,
    pub token: String,
    pub token_prefix: String,
    pub scopes: Vec<String>,
}

async fn require_admin_by_tenant_slug(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id =
        crate::repositories::membership_repository::resolve_tenant_id(pool, tenant_slug).await?;

    require_permission(pool, tenant_id, user_id, Permission::ManageApiTokens)
        .await
        .map(|ctx| ctx.tenant_id)
}

fn normalize_scopes(scopes: Option<&[String]>) -> Result<Vec<String>, AppError> {
    let scopes = scopes
        .map(|scopes| {
            scopes
                .iter()
                .map(|scope| scope.trim().to_string())
                .collect()
        })
        .unwrap_or_else(|| vec!["posts:read".to_string()]);

    if scopes.is_empty() || scopes.iter().any(|scope| scope.is_empty()) {
        return Err(AppError::Validation(
            "scopes must contain at least one valid scope".to_string(),
        ));
    }

    Ok(scopes)
}

pub async fn list_api_tokens(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Vec<api_token_repository::ApiTokenListItemRow>, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;
    api_token_repository::list_api_tokens(pool, tenant_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, "error listing api tokens");
            AppError::InternalServerError
        })
}

pub async fn create_api_token(
    pool: &DbPool,
    tenant_slug: &str,
    name: &str,
    scopes: Option<&[String]>,
    user_id: Uuid,
) -> Result<ApiTokenCreatedDto, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;

    let scopes = normalize_scopes(scopes)?;
    let (token, prefix, _raw) = api_token_repository::generate_token();
    let token_hash = api_token_repository::hash_token(&token);

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error starting api token create transaction");
        AppError::InternalServerError
    })?;

    let created = api_token_repository::create_api_token(&mut tx, tenant_id, name, &prefix, &token_hash, user_id, &scopes)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error creating api token");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "api_token",
            entity_id: created,
            action: audit::API_TOKEN_CREATED,
            old_value: None,
            new_value: Some(json!({
                "name": name,
                "token_prefix": prefix,
                "scopes": scopes,
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error committing api token create transaction");
        AppError::InternalServerError
    })?;

    Ok(ApiTokenCreatedDto {
        id: created,
        name: name.to_string(),
        token,
        token_prefix: prefix,
        scopes,
    })
}

pub async fn revoke_api_token(
    pool: &DbPool,
    token_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let token = api_token_repository::get_api_token(pool, token_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, token_id = %token_id, "error fetching api token");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, token.tenant_id, user_id, Permission::ManageApiTokens).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, token_id = %token_id, user_id = %user_id, "error starting api token revoke transaction");
        AppError::InternalServerError
    })?;

    api_token_repository::revoke_api_token(&mut tx, token_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, token_id = %token_id, user_id = %user_id, "error revoking api token");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id: token.tenant_id,
            actor_user_id: user_id,
            entity_type: "api_token",
            entity_id: token_id,
            action: audit::API_TOKEN_REVOKED,
            old_value: Some(json!({
                "name": token.name,
                "token_prefix": token.token_prefix,
                "revoked_at": token.revoked_at,
                "scopes": token.scopes,
            })),
            new_value: Some(json!({
                "name": token.name,
                "token_prefix": token.token_prefix,
                "revoked": true,
                "scopes": token.scopes,
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, token_id = %token_id, user_id = %user_id, "error committing api token revoke transaction");
        AppError::InternalServerError
    })?;

    Ok(())
}
