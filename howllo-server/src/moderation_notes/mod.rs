use actix_web::{get, post, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::services::moderation_note_service;

#[derive(Debug, Deserialize)]
pub struct CreateModerationNoteRequest {
    pub body: String,
}

#[post("/api/admin/posts/{post_id}/notes")]
pub async fn create_post_note(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<CreateModerationNoteRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();

    if body.body.trim().is_empty() {
        return Err(AppError::Validation("body is required".to_string()));
    }

    moderation_note_service::create_post_note(pool.get_ref(), post_id, auth.0.id, body.body.trim())
        .await?;

    Ok(HttpResponse::Created().finish())
}

#[get("/api/admin/posts/{post_id}/notes")]
pub async fn list_post_notes(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    let items =
        moderation_note_service::list_post_notes(pool.get_ref(), post_id, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(items))
}

#[post("/api/admin/comments/{comment_id}/notes")]
pub async fn create_comment_note(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<CreateModerationNoteRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let comment_id = path.into_inner();

    if body.body.trim().is_empty() {
        return Err(AppError::Validation("body is required".to_string()));
    }

    moderation_note_service::create_comment_note(
        pool.get_ref(),
        comment_id,
        auth.0.id,
        body.body.trim(),
    )
    .await?;

    Ok(HttpResponse::Created().finish())
}

#[get("/api/admin/comments/{comment_id}/notes")]
pub async fn list_comment_notes(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let comment_id = path.into_inner();
    let items =
        moderation_note_service::list_comment_notes(pool.get_ref(), comment_id, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(items))
}

#[cfg(test)]
mod tests;
