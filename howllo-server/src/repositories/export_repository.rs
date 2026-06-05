use uuid::Uuid;

use crate::db::DbPool;

pub struct ExportPostRecord {
    pub id: Uuid,
    pub board_slug: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub vote_count: i32,
    pub is_hidden: bool,
    pub duplicate_of_post_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn fetch_export_posts(
    pool: &DbPool,
    tenant_id: Uuid,
) -> Result<Vec<ExportPostRecord>, sqlx::Error> {
    sqlx::query_as!(
        ExportPostRecord,
        r#"
        SELECT p.id, b.slug as board_slug, p.title, p.body, p.status, p.vote_count, p.is_hidden, p.duplicate_of_post_id, p.created_at
        FROM posts p
        JOIN boards b ON b.id = p.board_id
        WHERE p.tenant_id = $1
        ORDER BY p.created_at DESC
        "#,
        tenant_id
    )
    .fetch_all(pool)
    .await
}
