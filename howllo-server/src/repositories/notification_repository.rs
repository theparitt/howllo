use uuid::Uuid;

use crate::db::DbPool;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct NotificationRow {
    pub id: Uuid,
    pub event_type: String,
    pub title: String,
    pub body: String,
    pub is_read: bool,
    pub post_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[allow(clippy::too_many_arguments)]
pub async fn create_follow_notifications(
    pool: &DbPool,
    tenant_id: Uuid,
    post_id: Uuid,
    event_type: &str,
    title: &str,
    body: &str,
    actor_user_id: Uuid,
    notify_column: &str,
) -> Result<(), sqlx::Error> {
    let query = format!(
        r#"
        INSERT INTO notifications (tenant_id, user_id, post_id, event_type, title, body)
        SELECT $1, pf.user_id, $2, $3, $4, $5
        FROM post_follows pf
        WHERE pf.post_id = $2
          AND pf.user_id != $6
          AND pf.{notify_column} = TRUE
        "#,
    );

    sqlx::query(&query)
        .bind(tenant_id)
        .bind(post_id)
        .bind(event_type)
        .bind(title)
        .bind(body)
        .bind(actor_user_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_notifications(
    pool: &DbPool,
    user_id: Uuid,
) -> Result<Vec<NotificationRow>, sqlx::Error> {
    sqlx::query_as!(
        NotificationRow,
        r#"
        SELECT id, event_type, title, body, is_read, post_id, created_at
        FROM notifications
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 100
        "#,
        user_id
    )
    .fetch_all(pool)
    .await
}

pub async fn mark_notification_read(
    pool: &DbPool,
    notification_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE notifications SET is_read = TRUE WHERE id = $1 AND user_id = $2",
        notification_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
