use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPool;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct AiSuggestionRow {
    pub id: Uuid,
    pub suggestion_type: String,
    pub status: String,
    pub payload: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_ai_suggestions(
    pool: &DbPool,
    tenant_id: Uuid,
) -> Result<Vec<AiSuggestionRow>, sqlx::Error> {
    sqlx::query_as::<_, AiSuggestionRow>(
        "SELECT id, suggestion_type, status, payload, created_at, reviewed_at FROM ai_suggestions WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

pub async fn get_post_source(
    pool: &DbPool,
    post_id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT title, body FROM posts WHERE id = $1 AND tenant_id = $2",
        post_id,
        tenant_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| (row.title, row.body)))
}

pub async fn get_post_title(
    pool: &DbPool,
    post_id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT title FROM posts WHERE id = $1 AND tenant_id = $2",
        post_id,
        tenant_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| row.title))
}

pub async fn get_thread_comments(pool: &DbPool, post_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT body FROM comments WHERE post_id = $1 AND is_hidden = false ORDER BY created_at ASC LIMIT 10",
        post_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.body).collect())
}

pub async fn insert_ai_suggestion(
    pool: &DbPool,
    tenant_id: Uuid,
    post_id: Uuid,
    suggestion_type: &str,
    payload: Value,
    created_by_user_id: Uuid,
) -> Result<AiSuggestionRow, sqlx::Error> {
    sqlx::query_as::<_, AiSuggestionRow>(
        "INSERT INTO ai_suggestions (tenant_id, post_id, suggestion_type, payload, created_by_user_id) VALUES ($1, $2, $3, $4, $5) RETURNING id, suggestion_type, status, payload, created_at, reviewed_at",
    )
    .bind(tenant_id)
    .bind(post_id)
    .bind(suggestion_type)
    .bind(payload)
    .bind(created_by_user_id)
    .fetch_one(pool)
    .await
}

pub async fn update_ai_suggestion_status(
    pool: &DbPool,
    suggestion_id: Uuid,
    tenant_id: Uuid,
    status: &str,
    reviewed_by: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE ai_suggestions SET status = $1, reviewed_by_user_id = $2, reviewed_at = NOW() WHERE id = $3 AND tenant_id = $4",
    )
    .bind(status)
    .bind(reviewed_by)
    .bind(suggestion_id)
    .bind(tenant_id)
    .execute(pool)
    .await?;
    Ok(())
}
