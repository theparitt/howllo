use uuid::Uuid;

use crate::comments::models::CommentType;
use crate::db::DbPool;
use crate::dto::{CommentCreatedDto, CommentListItemDto};

pub struct CommentPostContextRecord {
    pub tenant_id: Uuid,
    pub post_id: Uuid,
    pub title: String,
}

pub async fn get_comment_post_context(
    pool: &DbPool,
    comment_id: Uuid,
) -> Result<Option<CommentPostContextRecord>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT p.tenant_id, p.id as post_id, p.title
        FROM comments c
        JOIN posts p ON c.post_id = p.id
        WHERE c.id = $1
        "#,
        comment_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| CommentPostContextRecord {
        tenant_id: row.tenant_id,
        post_id: row.post_id,
        title: row.title,
    }))
}

pub async fn update_comment_official(
    pool: &DbPool,
    comment_id: Uuid,
    is_official: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE comments SET is_official_response = $1, comment_type = $2, updated_at = NOW() WHERE id = $3",
        is_official,
        if is_official {
            CommentType::Official.as_db_str()
        } else {
            CommentType::User.as_db_str()
        },
        comment_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_comment_official_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    comment_id: Uuid,
    is_official: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE comments SET is_official_response = $1, comment_type = $2, updated_at = NOW() WHERE id = $3",
        is_official,
        if is_official {
            CommentType::Official.as_db_str()
        } else {
            CommentType::User.as_db_str()
        },
        comment_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn get_comment_tenant(
    pool: &DbPool,
    comment_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT p.tenant_id
        FROM comments c
        JOIN posts p ON c.post_id = p.id
        WHERE c.id = $1
        "#,
        comment_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| row.tenant_id))
}

pub async fn get_comment_visibility(
    pool: &DbPool,
    comment_id: Uuid,
) -> Result<Option<bool>, sqlx::Error> {
    let row = sqlx::query!("SELECT is_hidden FROM comments WHERE id = $1", comment_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.is_hidden))
}

pub async fn get_comment_official_state(
    pool: &DbPool,
    comment_id: Uuid,
) -> Result<Option<bool>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT is_official_response FROM comments WHERE id = $1",
        comment_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| row.is_official_response))
}

pub async fn update_comment_visibility(
    pool: &DbPool,
    comment_id: Uuid,
    is_hidden: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE comments SET is_hidden = $1, updated_at = NOW() WHERE id = $2",
        is_hidden,
        comment_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_comment_visibility_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    comment_id: Uuid,
    is_hidden: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE comments SET is_hidden = $1, updated_at = NOW() WHERE id = $2",
        is_hidden,
        comment_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn create_comment(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
    body: &str,
) -> Result<CommentCreatedDto, sqlx::Error> {
    sqlx::query_as!(
        CommentCreatedDto,
        r#"
        INSERT INTO comments (post_id, user_id, body)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
        post_id,
        user_id,
        body
    )
    .fetch_one(pool)
    .await
}

pub async fn list_visible_comments(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Vec<CommentListItemDto>, sqlx::Error> {
    sqlx::query_as!(
        CommentListItemDto,
        r#"
        SELECT c.id, c.body, c.is_official_response, c.comment_type, c.created_at, u.display_name
        FROM comments c
        JOIN users u ON c.user_id = u.id
        WHERE c.post_id = $1 AND c.is_hidden = false
        ORDER BY c.created_at ASC
        "#,
        post_id
    )
    .fetch_all(pool)
    .await
}

pub async fn count_visible_comments(pool: &DbPool, post_id: Uuid) -> Result<i64, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT count(*)::bigint AS count FROM comments WHERE post_id = $1 AND is_hidden = false",
        post_id
    )
    .fetch_one(pool)
    .await?;
    Ok(row.count.unwrap_or(0))
}

pub async fn list_visible_comments_paginated(
    pool: &DbPool,
    post_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<CommentListItemDto>, sqlx::Error> {
    sqlx::query_as!(
        CommentListItemDto,
        r#"
        SELECT c.id, c.body, c.is_official_response, c.comment_type, c.created_at, u.display_name
        FROM comments c
        JOIN users u ON c.user_id = u.id
        WHERE c.post_id = $1 AND c.is_hidden = false
        ORDER BY c.created_at ASC
        LIMIT $2 OFFSET $3
        "#,
        post_id,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
}

pub struct PostAccessForComments {
    pub tenant_id: Uuid,
    pub is_private: bool,
    pub is_hidden: bool,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn get_post_access_for_comments(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Option<PostAccessForComments>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT p.tenant_id, b.is_private, p.is_hidden, p.deleted_at
        FROM posts p
        JOIN boards b ON p.board_id = b.id
        WHERE p.id = $1
        "#,
        post_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| PostAccessForComments {
        tenant_id: row.tenant_id,
        is_private: row.is_private,
        is_hidden: row.is_hidden,
        deleted_at: row.deleted_at,
    }))
}
