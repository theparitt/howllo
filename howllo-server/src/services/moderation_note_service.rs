use uuid::Uuid;

use crate::auth::require_permission;
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::errors::AppError;
use crate::repositories::{comment_repository, moderation_note_repository, post_repository};

async fn require_admin_by_post(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id = post_repository::get_post_tenant(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, "error fetching post for moderation note");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::AddModerationNote)
        .await
        .map(|ctx| ctx.tenant_id)
}

async fn require_admin_by_comment(
    pool: &DbPool,
    comment_id: Uuid,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id = comment_repository::get_comment_tenant(pool, comment_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, comment_id = %comment_id, "error fetching comment for moderation note");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::AddModerationNote)
        .await
        .map(|ctx| ctx.tenant_id)
}

pub async fn create_post_note(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
    body: &str,
) -> Result<(), AppError> {
    let tenant_id = require_admin_by_post(pool, post_id, user_id).await?;

    moderation_note_repository::create_post_note(pool, tenant_id, post_id, user_id, body)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error creating post moderation note");
            AppError::InternalServerError
        })
}

pub async fn list_post_notes(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<moderation_note_repository::ModerationNoteRow>, AppError> {
    require_admin_by_post(pool, post_id, user_id).await?;

    moderation_note_repository::list_post_notes(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error listing post moderation notes");
            AppError::InternalServerError
        })
}

pub async fn create_comment_note(
    pool: &DbPool,
    comment_id: Uuid,
    user_id: Uuid,
    body: &str,
) -> Result<(), AppError> {
    let tenant_id = require_admin_by_comment(pool, comment_id, user_id).await?;

    moderation_note_repository::create_comment_note(pool, tenant_id, comment_id, user_id, body)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error creating comment moderation note");
            AppError::InternalServerError
        })
}

pub async fn list_comment_notes(
    pool: &DbPool,
    comment_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<moderation_note_repository::ModerationNoteRow>, AppError> {
    require_admin_by_comment(pool, comment_id, user_id).await?;

    moderation_note_repository::list_comment_notes(pool, comment_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error listing comment moderation notes");
            AppError::InternalServerError
        })
}
