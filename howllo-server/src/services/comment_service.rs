use actix_web::HttpRequest;
use uuid::Uuid;

use crate::auth::maybe_authenticated_user;
use crate::db::DbPool;
use crate::dto::{CommentCreatedDto, CommentListItemDto};
use crate::errors::AppError;
use crate::memberships;
use crate::notifications::{self, FollowNotification};
use crate::repositories::{comment_repository, subscription_repository};
use crate::services::post_service;

fn snippet(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.chars().count() > 140 {
        format!("{}…", trimmed.chars().take(139).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

pub async fn create_comment(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
    body: &str,
) -> Result<CommentCreatedDto, AppError> {
    let access = post_service::ensure_post_interaction_access(pool, post_id, user_id, true).await?;

    let created = comment_repository::create_comment(pool, post_id, user_id, body)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error creating comment");
            AppError::InternalServerError
        })?;

    // The commenter now follows the post (never resets existing prefs).
    let _ = subscription_repository::ensure_follow(pool, post_id, user_id).await;

    // Notify everyone following the post (author included) except the commenter.
    let title: Option<String> = sqlx::query_scalar("SELECT title FROM posts WHERE id = $1")
        .bind(post_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    let heading = match title {
        Some(t) => format!("New comment on \"{}\"", snippet(&t)),
        None => "New comment".to_string(),
    };
    let _ = notifications::create_follow_notifications(
        pool,
        access.tenant_id,
        post_id,
        user_id,
        FollowNotification {
            event_type: "post_comment",
            title: &heading,
            body: &snippet(body),
            notify_column: "notify_on_comment",
        },
    )
    .await;

    Ok(created)
}

pub async fn list_comments(
    req: &HttpRequest,
    pool: &DbPool,
    post_id: Uuid,
    page: Option<i64>,
    per_page: Option<i64>,
) -> Result<Vec<CommentListItemDto>, AppError> {
    let access = comment_repository::get_post_access_for_comments(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, "error fetching post access for comments");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    if access.is_hidden || access.deleted_at.is_some() {
        return Err(AppError::NotFound);
    }

    if access.is_private {
        let user = maybe_authenticated_user(req)
            .await?
            .ok_or(AppError::Forbidden)?;

        memberships::check_membership(pool, access.tenant_id, user.id)
            .await
            .map_err(|_| AppError::Forbidden)?;
    }

    let (limit, offset) = match (page, per_page) {
        (Some(p), Some(pp)) => {
            let p = p.max(1);
            let pp = pp.clamp(1, 100);
            (pp, (p - 1) * pp)
        }
        _ => {
            return comment_repository::list_visible_comments(pool, post_id)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, post_id = %post_id, "error fetching comments");
                    AppError::InternalServerError
                });
        }
    };

    comment_repository::list_visible_comments_paginated(pool, post_id, limit, offset)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, "error fetching paginated comments");
            AppError::InternalServerError
        })
}
