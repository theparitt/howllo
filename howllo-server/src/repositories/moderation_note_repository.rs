use crate::db::DbPool;
use uuid::Uuid;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct ModerationNoteRow {
    pub id: Uuid,
    pub body: String,
    pub author_display_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_post_note(
    pool: &DbPool,
    tenant_id: Uuid,
    post_id: Uuid,
    author_user_id: Uuid,
    body: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO moderation_notes (tenant_id, post_id, author_user_id, body) VALUES ($1, $2, $3, $4)",
        tenant_id,
        post_id,
        author_user_id,
        body
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn create_comment_note(
    pool: &DbPool,
    tenant_id: Uuid,
    comment_id: Uuid,
    author_user_id: Uuid,
    body: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO moderation_notes (tenant_id, comment_id, author_user_id, body) VALUES ($1, $2, $3, $4)",
        tenant_id,
        comment_id,
        author_user_id,
        body
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_post_notes(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Vec<ModerationNoteRow>, sqlx::Error> {
    sqlx::query_as!(
        ModerationNoteRow,
        r#"
        SELECT n.id, n.body, u.display_name as author_display_name, n.created_at
        FROM moderation_notes n
        JOIN users u ON u.id = n.author_user_id
        WHERE n.post_id = $1
        ORDER BY n.created_at DESC
        "#,
        post_id
    )
    .fetch_all(pool)
    .await
}

pub async fn list_comment_notes(
    pool: &DbPool,
    comment_id: Uuid,
) -> Result<Vec<ModerationNoteRow>, sqlx::Error> {
    sqlx::query_as!(
        ModerationNoteRow,
        r#"
        SELECT n.id, n.body, u.display_name as author_display_name, n.created_at
        FROM moderation_notes n
        JOIN users u ON u.id = n.author_user_id
        WHERE n.comment_id = $1
        ORDER BY n.created_at DESC
        "#,
        comment_id
    )
    .fetch_all(pool)
    .await
}
