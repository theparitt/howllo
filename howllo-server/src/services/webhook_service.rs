use serde_json::json;
use uuid::Uuid;

use crate::audit::{self, record_in_tx, AuditEntry};
use crate::auth::require_permission;
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::errors::AppError;
use crate::repositories::webhook_repository;

async fn require_admin_by_tenant_slug(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id =
        crate::repositories::membership_repository::resolve_tenant_id(pool, tenant_slug).await?;

    require_permission(pool, tenant_id, user_id, Permission::ManageWebhooks)
        .await
        .map(|ctx| ctx.tenant_id)
}

#[derive(Debug, serde::Serialize)]
pub struct WebhookEndpointDto {
    pub id: Uuid,
    pub url: String,
    pub has_secret: bool,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_webhooks(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Vec<WebhookEndpointDto>, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;
    let items = webhook_repository::list_webhooks(pool, tenant_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, "error listing webhooks");
            AppError::InternalServerError
        })?;

    Ok(items
        .into_iter()
        .map(|item| WebhookEndpointDto {
            id: item.id,
            url: item.url,
            has_secret: item
                .secret
                .as_ref()
                .is_some_and(|secret| !secret.is_empty()),
            is_active: item.is_active,
            created_at: item.created_at,
        })
        .collect())
}

pub async fn create_webhook(
    pool: &DbPool,
    tenant_slug: &str,
    url: &str,
    secret: Option<&str>,
    user_id: Uuid,
) -> Result<WebhookEndpointDto, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error starting webhook create transaction");
        AppError::InternalServerError
    })?;

    let item = webhook_repository::create_webhook(&mut tx, tenant_id, url, secret)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error creating webhook");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "webhook",
            entity_id: item.id,
            action: audit::WEBHOOK_CREATED,
            old_value: None,
            new_value: Some(json!({
                "url": item.url,
                "has_secret": item.secret.as_ref().is_some_and(|secret| !secret.is_empty()),
                "is_active": item.is_active,
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error committing webhook create transaction");
        AppError::InternalServerError
    })?;

    Ok(WebhookEndpointDto {
        id: item.id,
        url: item.url,
        has_secret: item
            .secret
            .as_ref()
            .is_some_and(|secret| !secret.is_empty()),
        is_active: item.is_active,
        created_at: item.created_at,
    })
}

pub async fn deactivate_webhook(
    pool: &DbPool,
    webhook_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let tenant_id = webhook_repository::get_webhook_tenant(pool, webhook_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, webhook_id = %webhook_id, "error fetching webhook endpoint");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::ManageWebhooks).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, webhook_id = %webhook_id, user_id = %user_id, "error starting webhook deactivate transaction");
        AppError::InternalServerError
    })?;

    webhook_repository::deactivate_webhook(&mut tx, webhook_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, webhook_id = %webhook_id, user_id = %user_id, "error deactivating webhook");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "webhook",
            entity_id: webhook_id,
            action: audit::WEBHOOK_DEACTIVATED,
            old_value: Some(json!({ "is_active": true })),
            new_value: Some(json!({ "is_active": false })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, webhook_id = %webhook_id, user_id = %user_id, "error committing webhook deactivate transaction");
        AppError::InternalServerError
    })?;

    Ok(())
}
