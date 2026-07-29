use uuid::Uuid;

use crate::db::DbPool;

pub struct FollowStateRow {
    pub notify_on_status_change: bool,
    pub notify_on_official_response: bool,
}

pub async fn upsert_follow(pool: &DbPool, post_id: Uuid, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO post_follows (post_id, user_id)
        VALUES ($1, $2)
        ON CONFLICT (post_id, user_id)
        DO UPDATE SET
            notify_on_status_change = TRUE,
            notify_on_official_response = TRUE
        "#,
        post_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Ensure a follow exists without resetting the user's notify preferences.
/// Used for auto-following (author on post create, commenter on comment) so we
/// never clobber a follow the user has already customized. Untyped so no sqlx
/// cache regeneration is needed.
pub async fn ensure_follow(pool: &DbPool, post_id: Uuid, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO post_follows (post_id, user_id)
        VALUES ($1, $2)
        ON CONFLICT (post_id, user_id) DO NOTHING
        "#,
    )
    .bind(post_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_follow(pool: &DbPool, post_id: Uuid, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM post_follows WHERE post_id = $1 AND user_id = $2",
        post_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_follow_state(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<Option<FollowStateRow>, sqlx::Error> {
    sqlx::query!(
        r#"
        SELECT notify_on_status_change, notify_on_official_response
        FROM post_follows
        WHERE post_id = $1 AND user_id = $2
        "#,
        post_id,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map(|opt| {
        opt.map(|row| FollowStateRow {
            notify_on_status_change: row.notify_on_status_change,
            notify_on_official_response: row.notify_on_official_response,
        })
    })
}
