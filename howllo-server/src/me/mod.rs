use actix_web::{get, patch, web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;

#[derive(Debug, Serialize)]
pub struct CurrentUserDto {
    pub id: Uuid,
    pub rooiam_subject: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMeInput {
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MeActivityQuery {
    pub tenant_slug: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MePostActivityItem {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub vote_count: i32,
    pub comment_count: i64,
    pub board_slug: String,
    pub board_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MeCommentActivityItem {
    pub id: Uuid,
    pub post_id: Uuid,
    pub post_title: String,
    pub board_slug: String,
    pub board_name: String,
    pub body: String,
    pub comment_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MePostReferenceActivityItem {
    pub post_id: Uuid,
    pub post_title: String,
    pub board_slug: String,
    pub board_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MeStatusActivityItem {
    pub id: Uuid,
    pub post_id: Uuid,
    pub post_title: String,
    pub board_slug: String,
    pub board_name: String,
    pub old_status: Option<String>,
    pub new_status: String,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MeActivityDto {
    pub posts: Vec<MePostActivityItem>,
    pub comments: Vec<MeCommentActivityItem>,
    pub votes: Vec<MePostReferenceActivityItem>,
    pub follows: Vec<MePostReferenceActivityItem>,
    pub status_changes: Vec<MeStatusActivityItem>,
}

async fn resolve_tenant_id(pool: &DbPool, tenant_slug: &str) -> Result<Uuid, AppError> {
    let slug = tenant_slug.trim();
    if slug.is_empty() {
        return Err(AppError::Validation("tenant_slug is required".to_string()));
    }

    let tenant_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM tenants WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_slug = slug, "error resolving tenant for me activity");
            AppError::InternalServerError
        })?;

    tenant_id.ok_or(AppError::NotFound)
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceRoleQuery {
    pub tenant_slug: String,
}

/// The signed-in user's role in a workspace (owner/admin/moderator/member) or
/// null if they have no membership. Lets the web show a "Manage" surface only
/// to staff. Account owners/admins resolve as `owner` via the authz shortcut.
#[get("/api/me/workspace-role")]
pub async fn get_my_workspace_role(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<WorkspaceRoleQuery>,
) -> Result<impl Responder, AppError> {
    let tenant_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM tenants WHERE slug = $1")
            .bind(query.tenant_slug.trim())
            .fetch_optional(pool.get_ref())
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "error resolving tenant for role lookup");
                AppError::InternalServerError
            })?;

    let role = match tenant_id {
        Some(tid) => crate::auth::resolve_effective_role(pool.get_ref(), tid, auth.0.id).await,
        None => None,
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({ "role": role })))
}

#[get("/api/me")]
pub async fn get_me(auth: AuthenticatedUser) -> Result<impl Responder, AppError> {
    Ok(HttpResponse::Ok().json(CurrentUserDto {
        id: auth.0.id,
        rooiam_subject: auth.0.rooiam_subject,
        email: auth.0.email,
        display_name: auth.0.display_name,
        avatar_url: auth.0.avatar_url,
    }))
}

#[patch("/api/me")]
pub async fn update_me(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<UpdateMeInput>,
) -> Result<impl Responder, AppError> {
    let display_name = body.display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::Validation("display_name is required".to_string()));
    }
    if display_name.chars().count() > 100 {
        return Err(AppError::Validation(
            "display_name must be 100 characters or fewer".to_string(),
        ));
    }

    let avatar_url = body
        .avatar_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let user = sqlx::query_as::<_, crate::users::User>(
        r#"
        UPDATE users
        SET
            display_name = $2,
            avatar_url = $3,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, rooiam_subject, email, display_name, avatar_url, created_at, updated_at
        "#,
    )
    .bind(auth.0.id)
    .bind(display_name)
    .bind(avatar_url)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, user_id = %auth.0.id, "error updating current user profile");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Ok().json(CurrentUserDto {
        id: user.id,
        rooiam_subject: user.rooiam_subject,
        email: user.email,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
    }))
}

#[get("/api/me/activity")]
pub async fn get_me_activity(
    pool: web::Data<DbPool>,
    query: web::Query<MeActivityQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = resolve_tenant_id(pool.get_ref(), &query.tenant_slug).await?;
    let user_id = auth.0.id;

    let posts = sqlx::query_as::<_, MePostActivityItem>(
        r#"
        SELECT
            p.id,
            p.title,
            p.status,
            p.vote_count,
            (
                SELECT COUNT(*)::BIGINT
                FROM comments c
                WHERE c.post_id = p.id AND c.is_hidden = FALSE
            ) AS comment_count,
            b.slug AS board_slug,
            b.name AS board_name,
            p.created_at
        FROM posts p
        JOIN boards b ON b.id = p.board_id
        WHERE p.tenant_id = $1
          AND p.user_id = $2
          AND p.is_hidden = FALSE
          AND p.deleted_at IS NULL
        ORDER BY p.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error loading me posts");
        AppError::InternalServerError
    })?;

    let comments = sqlx::query_as::<_, MeCommentActivityItem>(
        r#"
        SELECT
            c.id,
            p.id AS post_id,
            p.title AS post_title,
            b.slug AS board_slug,
            b.name AS board_name,
            c.body,
            c.comment_type,
            c.created_at
        FROM comments c
        JOIN posts p ON p.id = c.post_id
        JOIN boards b ON b.id = p.board_id
        WHERE p.tenant_id = $1
          AND c.user_id = $2
          AND c.is_hidden = FALSE
          AND p.is_hidden = FALSE
          AND p.deleted_at IS NULL
        ORDER BY c.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error loading me comments");
        AppError::InternalServerError
    })?;

    let votes = sqlx::query_as::<_, MePostReferenceActivityItem>(
        r#"
        SELECT
            p.id AS post_id,
            p.title AS post_title,
            b.slug AS board_slug,
            b.name AS board_name,
            pv.created_at
        FROM post_votes pv
        JOIN posts p ON p.id = pv.post_id
        JOIN boards b ON b.id = p.board_id
        WHERE p.tenant_id = $1
          AND pv.user_id = $2
          AND p.is_hidden = FALSE
          AND p.deleted_at IS NULL
        ORDER BY pv.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error loading me votes");
        AppError::InternalServerError
    })?;

    let follows = sqlx::query_as::<_, MePostReferenceActivityItem>(
        r#"
        SELECT
            p.id AS post_id,
            p.title AS post_title,
            b.slug AS board_slug,
            b.name AS board_name,
            pf.created_at
        FROM post_follows pf
        JOIN posts p ON p.id = pf.post_id
        JOIN boards b ON b.id = p.board_id
        WHERE p.tenant_id = $1
          AND pf.user_id = $2
          AND p.is_hidden = FALSE
          AND p.deleted_at IS NULL
        ORDER BY pf.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error loading me follows");
        AppError::InternalServerError
    })?;

    let status_changes = sqlx::query_as::<_, MeStatusActivityItem>(
        r#"
        SELECT
            h.id,
            p.id AS post_id,
            p.title AS post_title,
            b.slug AS board_slug,
            b.name AS board_name,
            h.old_status,
            h.new_status,
            h.reason,
            h.created_at
        FROM post_status_history h
        JOIN posts p ON p.id = h.post_id
        JOIN boards b ON b.id = p.board_id
        WHERE p.tenant_id = $1
          AND p.user_id = $2
          AND p.is_hidden = FALSE
          AND p.deleted_at IS NULL
        ORDER BY h.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error loading me status changes");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Ok().json(MeActivityDto {
        posts,
        comments,
        votes,
        follows,
        status_changes,
    }))
}
