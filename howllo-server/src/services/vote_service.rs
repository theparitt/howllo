use uuid::Uuid;

use crate::db::DbPool;
use crate::errors::AppError;
use crate::repositories::vote_repository;
use crate::services::post_service;

pub async fn add_vote(pool: &DbPool, post_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let access = post_service::ensure_post_interaction_access(pool, post_id, user_id, true).await?;
    if !access.allow_votes {
        return Err(AppError::Forbidden);
    }

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error opening vote transaction");
        AppError::InternalServerError
    })?;

    let rows = vote_repository::insert_vote(&mut tx, post_id, user_id).await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error inserting vote");
        AppError::InternalServerError
    })?;

    if rows > 0 {
        vote_repository::increment_vote_count(&mut tx, post_id).await.map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error incrementing vote count");
            AppError::InternalServerError
        })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error committing vote transaction");
        AppError::InternalServerError
    })?;

    Ok(())
}

pub async fn remove_vote(pool: &DbPool, post_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    post_service::ensure_post_interaction_access(pool, post_id, user_id, true).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error opening unvote transaction");
        AppError::InternalServerError
    })?;

    let rows = vote_repository::delete_vote(&mut tx, post_id, user_id).await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error deleting vote");
        AppError::InternalServerError
    })?;

    if rows > 0 {
        vote_repository::decrement_vote_count(&mut tx, post_id).await.map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error decrementing vote count");
            AppError::InternalServerError
        })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error committing unvote transaction");
        AppError::InternalServerError
    })?;

    Ok(())
}
