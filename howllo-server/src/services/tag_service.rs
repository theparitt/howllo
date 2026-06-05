use serde_json::json;
use uuid::Uuid;

use crate::audit::{self, record_in_tx, AuditEntry};
use crate::auth::require_permission;
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::dto::TagDto;
use crate::errors::AppError;
use crate::repositories::{post_repository, tag_repository};

async fn require_admin_by_tenant_slug(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id =
        crate::repositories::membership_repository::resolve_tenant_id(pool, tenant_slug).await?;

    require_permission(pool, tenant_id, user_id, Permission::ManageTags)
        .await
        .map(|ctx| ctx.tenant_id)
}

async fn require_admin_by_tag_id(
    pool: &DbPool,
    tag_id: Uuid,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id = tag_repository::get_tag_tenant(pool, tag_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tag_id = %tag_id, "error fetching tag for admin check");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::ManageTags)
        .await
        .map(|ctx| ctx.tenant_id)
}

async fn require_admin_by_post_id(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id = post_repository::get_post_tenant(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, "error fetching post for tag admin check");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::ManageTags)
        .await
        .map(|ctx| ctx.tenant_id)
}

pub async fn list_tags(pool: &DbPool, tenant_slug: &str) -> Result<Vec<TagDto>, AppError> {
    tag_repository::list_tags_by_tenant_slug(pool, tenant_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_slug = tenant_slug, "error listing tags");
            AppError::InternalServerError
        })
}

pub async fn create_tag(
    pool: &DbPool,
    tenant_slug: &str,
    slug: &str,
    name: &str,
    color: Option<&str>,
    user_id: Uuid,
) -> Result<TagDto, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error starting tag create transaction");
        AppError::InternalServerError
    })?;

    let tag = tag_repository::create_tag(&mut tx, tenant_id, slug, name, color)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error creating tag");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "tag",
            entity_id: tag.id,
            action: audit::TAG_CREATED,
            old_value: None,
            new_value: Some(json!({
                "slug": tag.slug,
                "name": tag.name,
                "color": tag.color,
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error committing tag create transaction");
        AppError::InternalServerError
    })?;

    Ok(tag)
}

pub async fn update_tag(
    pool: &DbPool,
    tag_id: Uuid,
    name: &str,
    color: Option<&str>,
    user_id: Uuid,
) -> Result<TagDto, AppError> {
    let tenant_id = require_admin_by_tag_id(pool, tag_id, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, tag_id = %tag_id, user_id = %user_id, "error starting tag update transaction");
        AppError::InternalServerError
    })?;

    let previous = tag_repository::get_tag_by_id(pool, tag_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tag_id = %tag_id, user_id = %user_id, "error fetching tag before update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let tag = tag_repository::update_tag(&mut tx, tag_id, name, color)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tag_id = %tag_id, user_id = %user_id, "error updating tag");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "tag",
            entity_id: tag_id,
            action: audit::TAG_UPDATED,
            old_value: Some(json!({
                "slug": previous.slug,
                "name": previous.name,
                "color": previous.color,
            })),
            new_value: Some(json!({
                "slug": tag.slug,
                "name": tag.name,
                "color": tag.color,
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, tag_id = %tag_id, user_id = %user_id, "error committing tag update transaction");
        AppError::InternalServerError
    })?;

    Ok(tag)
}

pub async fn delete_tag(pool: &DbPool, tag_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let tenant_id = require_admin_by_tag_id(pool, tag_id, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, tag_id = %tag_id, user_id = %user_id, "error starting tag delete transaction");
        AppError::InternalServerError
    })?;

    let previous = tag_repository::get_tag_by_id(pool, tag_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tag_id = %tag_id, user_id = %user_id, "error fetching tag before delete");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    tag_repository::delete_tag(&mut tx, tag_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tag_id = %tag_id, user_id = %user_id, "error deleting tag");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "tag",
            entity_id: tag_id,
            action: audit::TAG_DELETED,
            old_value: Some(json!({
                "slug": previous.slug,
                "name": previous.name,
                "color": previous.color,
            })),
            new_value: None,
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, tag_id = %tag_id, user_id = %user_id, "error committing tag delete transaction");
        AppError::InternalServerError
    })?;

    Ok(())
}

pub async fn attach_tag_to_post(
    pool: &DbPool,
    post_id: Uuid,
    tag_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let post_tenant_id = require_admin_by_post_id(pool, post_id, user_id).await?;
    let tag_tenant_id = require_admin_by_tag_id(pool, tag_id, user_id).await?;

    if post_tenant_id != tag_tenant_id {
        return Err(AppError::Validation(
            "tag must belong to the same tenant as the post".to_string(),
        ));
    }

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, tag_id = %tag_id, user_id = %user_id, "error starting tag attach transaction");
        AppError::InternalServerError
    })?;

    tag_repository::attach_tag_to_post(&mut tx, post_id, tag_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, tag_id = %tag_id, user_id = %user_id, "error attaching tag to post");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id: post_tenant_id,
            actor_user_id: user_id,
            entity_type: "post_tag",
            entity_id: post_id,
            action: audit::TAG_ATTACHED,
            old_value: None,
            new_value: Some(json!({ "post_id": post_id, "tag_id": tag_id })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, tag_id = %tag_id, user_id = %user_id, "error committing tag attach transaction");
        AppError::InternalServerError
    })?;

    Ok(())
}

pub async fn detach_tag_from_post(
    pool: &DbPool,
    post_id: Uuid,
    tag_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let post_tenant_id = require_admin_by_post_id(pool, post_id, user_id).await?;
    let tag_tenant_id = require_admin_by_tag_id(pool, tag_id, user_id).await?;

    if post_tenant_id != tag_tenant_id {
        return Err(AppError::Validation(
            "tag must belong to the same tenant as the post".to_string(),
        ));
    }

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, tag_id = %tag_id, user_id = %user_id, "error starting tag detach transaction");
        AppError::InternalServerError
    })?;

    tag_repository::detach_tag_from_post(&mut tx, post_id, tag_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, tag_id = %tag_id, user_id = %user_id, "error detaching tag from post");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id: post_tenant_id,
            actor_user_id: user_id,
            entity_type: "post_tag",
            entity_id: post_id,
            action: audit::TAG_DETACHED,
            old_value: Some(json!({ "post_id": post_id, "tag_id": tag_id })),
            new_value: None,
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, tag_id = %tag_id, user_id = %user_id, "error committing tag detach transaction");
        AppError::InternalServerError
    })?;

    Ok(())
}
