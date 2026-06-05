use actix_web::HttpRequest;
use uuid::Uuid;

use crate::auth::maybe_authenticated_user;
use crate::db::DbPool;
use crate::dto::{CommentCreatedDto, CommentListItemDto};
use crate::errors::AppError;
use crate::memberships;
use crate::repositories::comment_repository;
use crate::services::post_service;

pub async fn create_comment(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
    body: &str,
) -> Result<CommentCreatedDto, AppError> {
    post_service::ensure_post_interaction_access(pool, post_id, user_id, true).await?;

    comment_repository::create_comment(pool, post_id, user_id, body)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error creating comment");
            AppError::InternalServerError
        })
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
