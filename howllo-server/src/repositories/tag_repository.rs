use uuid::Uuid;

use crate::db::DbPool;
use crate::dto::{TagDto, TagSummaryItemDto};

pub async fn list_tags_by_tenant_slug(
    pool: &DbPool,
    tenant_slug: &str,
) -> Result<Vec<TagDto>, sqlx::Error> {
    sqlx::query_as!(
        TagDto,
        r#"
        SELECT tags.id, tags.slug, tags.name, tags.color
        FROM tags
        JOIN tenants t ON tags.tenant_id = t.id
        WHERE t.slug = $1
        ORDER BY tags.name ASC
        "#,
        tenant_slug
    )
    .fetch_all(pool)
    .await
}

pub async fn create_tag(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    slug: &str,
    name: &str,
    color: Option<&str>,
) -> Result<TagDto, sqlx::Error> {
    sqlx::query_as!(
        TagDto,
        r#"
        INSERT INTO tags (tenant_id, slug, name, color)
        VALUES ($1, $2, $3, $4)
        RETURNING id, slug, name, color
        "#,
        tenant_id,
        slug,
        name,
        color
    )
    .fetch_one(&mut **tx)
    .await
}

pub struct PreviousTag {
    pub slug: String,
    pub name: String,
    pub color: Option<String>,
}

pub async fn get_tag_by_id(
    pool: &DbPool,
    tag_id: Uuid,
) -> Result<Option<PreviousTag>, sqlx::Error> {
    let row = sqlx::query!("SELECT slug, name, color FROM tags WHERE id = $1", tag_id)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|row| PreviousTag {
        slug: row.slug,
        name: row.name,
        color: row.color,
    }))
}

pub async fn update_tag(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tag_id: Uuid,
    name: &str,
    color: Option<&str>,
) -> Result<TagDto, sqlx::Error> {
    sqlx::query_as!(
        TagDto,
        r#"
        UPDATE tags
        SET name = $1, color = $2
        WHERE id = $3
        RETURNING id, slug, name, color
        "#,
        name,
        color,
        tag_id
    )
    .fetch_one(&mut **tx)
    .await
}

pub async fn delete_tag(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tag_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM tags WHERE id = $1", tag_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn get_tag_tenant(pool: &DbPool, tag_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query!("SELECT tenant_id FROM tags WHERE id = $1", tag_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.tenant_id))
}

pub async fn attach_tag_to_post(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
    tag_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO post_tags (post_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        post_id,
        tag_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn detach_tag_from_post(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
    tag_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM post_tags WHERE post_id = $1 AND tag_id = $2",
        post_id,
        tag_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn get_tag_summary(
    pool: &DbPool,
    tenant_id: Uuid,
) -> Result<Vec<TagSummaryItemDto>, sqlx::Error> {
    sqlx::query_as!(
        TagSummaryItemDto,
        r#"
        SELECT t.id AS tag_id, t.slug AS tag_slug, t.name AS tag_name, coalesce(count(pt.post_id), 0)::bigint AS usage_count
        FROM tags t
        LEFT JOIN post_tags pt ON pt.tag_id = t.id
        WHERE t.tenant_id = $1
        GROUP BY t.id, t.slug, t.name
        ORDER BY usage_count DESC
        "#,
        tenant_id
    )
    .fetch_all(pool)
    .await
}
