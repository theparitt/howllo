use uuid::Uuid;

use crate::db::DbPool;
use crate::dto::PostFollowStateDto;
use crate::errors::AppError;
use crate::repositories::subscription_repository;
use crate::services::post_service;

pub async fn follow_post(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<PostFollowStateDto, AppError> {
    post_service::ensure_post_interaction_access(pool, post_id, user_id, false).await?;

    subscription_repository::upsert_follow(pool, post_id, user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error following post");
            AppError::InternalServerError
        })?;

    Ok(PostFollowStateDto {
        is_following: true,
        notify_on_status_change: true,
        notify_on_official_response: true,
    })
}

pub async fn unfollow_post(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<PostFollowStateDto, AppError> {
    post_service::ensure_post_interaction_access(pool, post_id, user_id, false).await?;

    subscription_repository::delete_follow(pool, post_id, user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error unfollowing post");
            AppError::InternalServerError
        })?;

    Ok(PostFollowStateDto {
        is_following: false,
        notify_on_status_change: false,
        notify_on_official_response: false,
    })
}

pub async fn get_follow_state(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<PostFollowStateDto, AppError> {
    post_service::ensure_post_interaction_access(pool, post_id, user_id, false).await?;

    let follow = subscription_repository::get_follow_state(pool, post_id, user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching follow state");
            AppError::InternalServerError
        })?;

    match follow {
        Some(row) => Ok(PostFollowStateDto {
            is_following: true,
            notify_on_status_change: row.notify_on_status_change,
            notify_on_official_response: row.notify_on_official_response,
        }),
        None => Ok(PostFollowStateDto {
            is_following: false,
            notify_on_status_change: false,
            notify_on_official_response: false,
        }),
    }
}
