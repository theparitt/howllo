use serde_json::json;
use uuid::Uuid;

use crate::audit::{self, record_in_tx, AuditEntry};
use crate::auth::require_permission;
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::errors::AppError;
use crate::repositories::export_repository;

async fn require_admin_by_tenant_slug(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id =
        crate::repositories::membership_repository::resolve_tenant_id(pool, tenant_slug).await?;

    require_permission(pool, tenant_id, user_id, Permission::ExportData)
        .await
        .map(|ctx| ctx.tenant_id)
}

#[derive(Debug, serde::Serialize)]
pub struct ExportPostDto {
    pub id: Uuid,
    pub board_slug: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub vote_count: i32,
    pub is_hidden: bool,
    pub duplicate_of_post_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn export_posts_json(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Vec<ExportPostDto>, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;
    let posts = export_repository::fetch_export_posts(pool, tenant_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, "error exporting posts");
            AppError::InternalServerError
        })?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error starting export json audit transaction");
        AppError::InternalServerError
    })?;
    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "tenant_export",
            entity_id: tenant_id,
            action: audit::EXPORT_POSTS_JSON,
            old_value: None,
            new_value: Some(json!({ "post_count": posts.len() })),
            reason: None,
        },
    )
    .await?;
    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error committing export json audit transaction");
        AppError::InternalServerError
    })?;

    let dtos = posts
        .into_iter()
        .map(|p| ExportPostDto {
            id: p.id,
            board_slug: p.board_slug,
            title: p.title,
            body: p.body,
            status: p.status,
            vote_count: p.vote_count,
            is_hidden: p.is_hidden,
            duplicate_of_post_id: p.duplicate_of_post_id,
            created_at: p.created_at,
        })
        .collect();

    Ok(dtos)
}

pub async fn export_posts_csv(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Vec<ExportPostDto>, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;
    let posts = export_repository::fetch_export_posts(pool, tenant_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, "error exporting posts");
            AppError::InternalServerError
        })?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error starting export csv audit transaction");
        AppError::InternalServerError
    })?;
    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "tenant_export",
            entity_id: tenant_id,
            action: audit::EXPORT_POSTS_CSV,
            old_value: None,
            new_value: Some(json!({ "post_count": posts.len() })),
            reason: None,
        },
    )
    .await?;
    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error committing export csv audit transaction");
        AppError::InternalServerError
    })?;

    let dtos = posts
        .into_iter()
        .map(|p| ExportPostDto {
            id: p.id,
            board_slug: p.board_slug,
            title: p.title,
            body: p.body,
            status: p.status,
            vote_count: p.vote_count,
            is_hidden: p.is_hidden,
            duplicate_of_post_id: p.duplicate_of_post_id,
            created_at: p.created_at,
        })
        .collect();

    Ok(dtos)
}
