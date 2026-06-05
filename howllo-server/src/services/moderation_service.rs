use serde_json::json;
use uuid::Uuid;

use crate::audit::{record_in_tx, AuditEntry};
use crate::auth::require_permission;
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::domain::status::{self, FeedbackStatus};
use crate::dto::{
    ModerationQueueItemDto, UpdateCommentVisibilityRequest, UpdateLockRequest,
    UpdateOfficialCommentRequest, UpdatePostDuplicateRequest, UpdatePostStatusRequest,
    UpdateVisibilityRequest,
};
use crate::errors::AppError;
use crate::notifications::{self, FollowNotification};
use crate::realtime::{Hub, RealtimeEvent};
use crate::repositories::{comment_repository, post_repository};
use crate::webhooks;

pub async fn get_moderation_queue(
    pool: &DbPool,
    tenant_slug: &str,
    board_slug: Option<&str>,
    page: i64,
    per_page: i64,
    user_id: Uuid,
) -> Result<(Vec<ModerationQueueItemDto>, i64), AppError> {
    let tenant_id =
        crate::repositories::membership_repository::resolve_tenant_id(pool, tenant_slug).await?;

    require_permission(pool, tenant_id, user_id, Permission::ModerateContent).await?;

    let total = post_repository::count_moderation_queue(pool, tenant_id, board_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, "error counting moderation queue");
            AppError::InternalServerError
        })?;

    let offset = (page - 1) * per_page;
    let items = post_repository::list_moderation_queue(
        pool, tenant_id, board_slug, per_page, offset,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, "error listing moderation queue");
        AppError::InternalServerError
    })?;

    Ok((items, total))
}

pub async fn update_post_status(
    pool: &DbPool,
    hub: &Hub,
    post_id: Uuid,
    body: &UpdatePostStatusRequest,
    user_id: Uuid,
) -> Result<(), AppError> {
    body.validate()?;
    let status = body.parse_status()?;

    let post = post_repository::get_post_status_context(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post for status update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, post.tenant_id, user_id, Permission::ChangeStatus).await?;

    let old_status = post.status.clone();
    let old_fs = FeedbackStatus::parse(&old_status).map_err(|_| AppError::InternalServerError)?;

    let allowed = status::allowed_transitions(old_fs);
    if !allowed.contains(&status) {
        return Err(AppError::Validation(format!(
            "cannot transition from {} to {}",
            old_fs.as_db_str(),
            status.as_db_str()
        )));
    }

    if status == FeedbackStatus::Declined && body.reason.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err(AppError::Validation(
            "reason is required when moving status to declined".to_string(),
        ));
    }

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error starting status update transaction");
        AppError::InternalServerError
    })?;

    post_repository::update_post_status_tx(&mut tx, post_id, status.as_db_str())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error updating post status");
            AppError::InternalServerError
        })?;

    post_repository::insert_status_history_tx(
        &mut tx,
        post_id,
        user_id,
        &old_status,
        status.as_db_str(),
        body.reason.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error writing post status history");
        AppError::InternalServerError
    })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id: post.tenant_id,
            actor_user_id: user_id,
            entity_type: "post",
            entity_id: post_id,
            action: "post_status_changed",
            old_value: Some(json!({ "status": old_status })),
            new_value: Some(json!({ "status": status.as_db_str() })),
            reason: body.reason.clone(),
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error committing status update transaction");
        AppError::InternalServerError
    })?;

    let notification_title = format!("Status changed: {}", post.title);
    let notification_body = format!(
        "Status moved from {} to {}{}",
        old_status,
        status.as_db_str(),
        body.reason
            .as_deref()
            .map(|reason| format!(" ({reason})"))
            .unwrap_or_default()
    );

    if let Err(e) = notifications::create_follow_notifications(
        pool,
        post.tenant_id,
        post_id,
        user_id,
        FollowNotification {
            event_type: "status_changed",
            title: &notification_title,
            body: &notification_body,
            notify_column: "notify_on_status_change",
        },
    )
    .await
    {
        tracing::warn!(error = %e, post_id = %post_id, user_id = %user_id, "failed to create status-change notifications");
    }

    if let Err(e) = webhooks::emit_tenant_event(
        pool,
        post.tenant_id,
        "post.status_changed",
        json!({
            "post_id": post_id,
            "old_status": old_status,
            "new_status": status.as_db_str(),
            "reason": body.reason,
        }),
    )
    .await
    {
        tracing::warn!(error = %e, post_id = %post_id, user_id = %user_id, "failed to emit status-change webhook");
    }

    hub.broadcast(
        post.tenant_id,
        RealtimeEvent {
            event_type: "post.status_changed".to_string(),
            board_id: None,
            post_id: Some(post_id),
        },
    );

    Ok(())
}

pub async fn update_post_visibility(
    pool: &DbPool,
    post_id: Uuid,
    body: &UpdateVisibilityRequest,
    user_id: Uuid,
) -> Result<(), AppError> {
    let tenant_id = post_repository::get_post_tenant(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post for visibility update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::HidePost).await?;

    let previous = post_repository::get_post_state(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post before visibility update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error starting post visibility transaction");
        AppError::InternalServerError
    })?;

    post_repository::update_post_visibility_tx(&mut tx, post_id, body.is_hidden)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error updating post visibility");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "post",
            entity_id: post_id,
            action: if body.is_hidden {
                "post_hidden"
            } else {
                "post_restored"
            },
            old_value: Some(json!({
                "is_hidden": previous.is_hidden,
                "deleted_at": previous.deleted_at,
            })),
            new_value: Some(json!({
                "is_hidden": body.is_hidden,
                "deleted_at": previous.deleted_at,
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error committing post visibility transaction");
        AppError::InternalServerError
    })
}

pub async fn soft_delete_post(pool: &DbPool, post_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let tenant_id = post_repository::get_post_tenant(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post for soft delete");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::HidePost).await?;

    let previous = post_repository::get_post_state(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post before soft delete");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error starting soft delete transaction");
        AppError::InternalServerError
    })?;

    post_repository::soft_delete_post_tx(&mut tx, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error soft deleting post");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "post",
            entity_id: post_id,
            action: "post_soft_deleted",
            old_value: Some(json!({
                "is_hidden": previous.is_hidden,
                "deleted_at": previous.deleted_at,
            })),
            new_value: Some(json!({
                "is_hidden": true,
                "deleted_at": "now",
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error committing soft delete transaction");
        AppError::InternalServerError
    })
}

pub async fn restore_post(pool: &DbPool, post_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let tenant_id = post_repository::get_post_tenant(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post for restore");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::HidePost).await?;

    let previous = post_repository::get_post_state(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post before restore");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error starting restore transaction");
        AppError::InternalServerError
    })?;

    post_repository::restore_post_tx(&mut tx, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error restoring post");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "post",
            entity_id: post_id,
            action: "post_restored",
            old_value: Some(json!({
                "is_hidden": previous.is_hidden,
                "deleted_at": previous.deleted_at,
            })),
            new_value: Some(json!({
                "is_hidden": false,
                "deleted_at": null,
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error committing restore transaction");
        AppError::InternalServerError
    })
}

pub async fn update_post_lock(
    pool: &DbPool,
    post_id: Uuid,
    body: &UpdateLockRequest,
    user_id: Uuid,
) -> Result<(), AppError> {
    let tenant_id = post_repository::get_post_tenant(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post for lock update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::LockPost).await?;

    let previous_is_locked = post_repository::get_post_lock(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post before lock update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error starting post lock transaction");
        AppError::InternalServerError
    })?;

    post_repository::update_post_lock_tx(&mut tx, post_id, body.is_locked)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error updating post lock");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "post",
            entity_id: post_id,
            action: "post_locked",
            old_value: Some(json!({ "is_locked": previous_is_locked })),
            new_value: Some(json!({ "is_locked": body.is_locked })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error committing post lock transaction");
        AppError::InternalServerError
    })
}

pub async fn update_comment_official(
    pool: &DbPool,
    comment_id: Uuid,
    body: &UpdateOfficialCommentRequest,
    user_id: Uuid,
) -> Result<(), AppError> {
    let context = comment_repository::get_comment_post_context(pool, comment_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error fetching comment/post for official flag update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(
        pool,
        context.tenant_id,
        user_id,
        Permission::ModerateContent,
    )
    .await?;

    let was_official = comment_repository::get_comment_official_state(pool, comment_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error fetching comment before official update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error starting comment official transaction");
        AppError::InternalServerError
    })?;

    comment_repository::update_comment_official_tx(&mut tx, comment_id, body.is_official)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error updating comment official flag");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id: context.tenant_id,
            actor_user_id: user_id,
            entity_type: "comment",
            entity_id: comment_id,
            action: "comment_marked_official",
            old_value: Some(json!({ "is_official_response": was_official })),
            new_value: Some(json!({ "is_official_response": body.is_official })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error committing comment official transaction");
        AppError::InternalServerError
    })?;

    if body.is_official {
        let notification_title = format!("Official response: {}", context.title);
        let notification_body = "A team member added an official response.".to_string();

        if let Err(e) = notifications::create_follow_notifications(
            pool,
            context.tenant_id,
            context.post_id,
            user_id,
            FollowNotification {
                event_type: "official_response",
                title: &notification_title,
                body: &notification_body,
                notify_column: "notify_on_official_response",
            },
        )
        .await
        {
            tracing::warn!(error = %e, comment_id = %comment_id, user_id = %user_id, "failed to create official-response notifications");
        }

        if let Err(e) = webhooks::emit_tenant_event(
            pool,
            context.tenant_id,
            "comment.official_response",
            json!({
                "comment_id": comment_id,
                "post_id": context.post_id,
            }),
        )
        .await
        {
            tracing::warn!(error = %e, comment_id = %comment_id, user_id = %user_id, "failed to emit official-response webhook");
        }
    }

    Ok(())
}

pub async fn update_comment_visibility(
    pool: &DbPool,
    comment_id: Uuid,
    body: &UpdateCommentVisibilityRequest,
    user_id: Uuid,
) -> Result<(), AppError> {
    let tenant_id = comment_repository::get_comment_tenant(pool, comment_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error fetching comment for visibility update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::HideComment).await?;

    let previous = comment_repository::get_comment_visibility(pool, comment_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error fetching comment before visibility update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error starting comment visibility transaction");
        AppError::InternalServerError
    })?;

    comment_repository::update_comment_visibility_tx(&mut tx, comment_id, body.is_hidden)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error updating comment visibility");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "comment",
            entity_id: comment_id,
            action: "comment_hidden",
            old_value: Some(json!({ "is_hidden": previous })),
            new_value: Some(json!({ "is_hidden": body.is_hidden })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, comment_id = %comment_id, user_id = %user_id, "error committing comment visibility transaction");
        AppError::InternalServerError
    })
}

pub async fn update_post_duplicate(
    pool: &DbPool,
    post_id: Uuid,
    body: &UpdatePostDuplicateRequest,
    user_id: Uuid,
) -> Result<(), AppError> {
    let post = post_repository::get_post_duplicate_context(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post for duplicate update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let tenant_id = post.tenant_id;

    require_permission(pool, tenant_id, user_id, Permission::MarkDuplicate).await?;

    if let Some(canonical_post_id) = body.duplicate_of_post_id {
        if canonical_post_id == post_id {
            return Err(AppError::Validation(
                "post cannot be marked as duplicate of itself".to_string(),
            ));
        }

        let canonical = post_repository::get_canonical_post_state(pool, canonical_post_id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, post_id = %post_id, canonical_post_id = %canonical_post_id, user_id = %user_id, "error fetching canonical post for duplicate update");
                AppError::InternalServerError
            })?;

        let canonical = canonical.ok_or(AppError::Validation(
            "canonical post was not found".to_string(),
        ))?;

        if canonical.tenant_id != tenant_id {
            return Err(AppError::Validation(
                "canonical post must belong to the same tenant".to_string(),
            ));
        }

        if canonical.is_hidden {
            return Err(AppError::Validation(
                "canonical post must not be hidden".to_string(),
            ));
        }

        if canonical.deleted_at.is_some() {
            return Err(AppError::Validation(
                "canonical post must not be deleted".to_string(),
            ));
        }

        if canonical.duplicate_of_post_id.is_some() {
            return Err(AppError::Validation(
                "canonical post is itself a duplicate".to_string(),
            ));
        }
    }

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error starting duplicate update transaction");
        AppError::InternalServerError
    })?;

    post_repository::update_duplicate_post_tx(&mut tx, post_id, body.duplicate_of_post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error updating duplicate post relation");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "post",
            entity_id: post_id,
            action: if body.duplicate_of_post_id.is_some() {
                "duplicate_marked"
            } else {
                "duplicate_removed"
            },
            old_value: Some(json!({ "duplicate_of_post_id": post.duplicate_of_post_id })),
            new_value: Some(json!({ "duplicate_of_post_id": body.duplicate_of_post_id })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error committing duplicate update transaction");
        AppError::InternalServerError
    })
}
