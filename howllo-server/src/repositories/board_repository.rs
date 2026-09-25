use sqlx::Row;
use uuid::Uuid;

use crate::db::DbPool;
use crate::dto::{BoardDetailDto, BoardListItemDto};

pub async fn list_public_boards(
    pool: &DbPool,
    tenant_slug: &str,
) -> Result<Vec<BoardListItemDto>, sqlx::Error> {
    sqlx::query_as::<_, BoardListItemDto>(
        r#"
        SELECT b.id, b.slug, b.name, b.description, b.board_type, b.icon_url, b.background_color, b.dashboard_sections, b.is_enabled
        FROM boards b
        JOIN tenants t ON b.tenant_id = t.id
        WHERE t.slug = $1 AND t.is_published = TRUE AND b.is_private = false AND b.is_enabled = true
        ORDER BY b.created_at ASC
        "#,
    )
    .bind(tenant_slug)
    .fetch_all(pool)
    .await
}

pub async fn ensure_default_board_for_tenant_slug(
    pool: &DbPool,
    tenant_slug: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO boards (tenant_id, slug, name, description, board_type, is_private, is_default)
        SELECT
            t.id,
            defaults.slug,
            defaults.name,
            defaults.description,
            defaults.board_type,
            FALSE,
            TRUE
        FROM tenants t
        CROSS JOIN (
            VALUES
                ('general', 'General', 'Catch-all board for ideas, bugs, feature requests, and general product discussion.', 'general'),
                ('feature-requests', 'Feature Requests', 'Vote on improvements, new capabilities, and product ideas.', 'feedback'),
                ('bug-reports', 'Bug Reports', 'Report broken flows, errors, regressions, and usability problems.', 'support')
        ) AS defaults(slug, name, description, board_type)
        WHERE t.slug = $1
          AND t.default_board_enabled = TRUE
        ON CONFLICT (tenant_id, slug) DO NOTHING
        "#,
    )
    .bind(tenant_slug)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_public_board_by_slug(
    pool: &DbPool,
    tenant_slug: &str,
    board_slug: &str,
) -> Result<Option<BoardDetailDto>, sqlx::Error> {
    sqlx::query_as::<_, BoardDetailDto>(
        r#"
        SELECT b.id, b.slug, b.name, b.description, b.board_type, b.is_private, b.is_enabled, b.icon_url, b.background_color, b.dashboard_sections
        FROM boards b
        JOIN tenants t ON b.tenant_id = t.id
        WHERE t.slug = $1 AND t.is_published = TRUE AND b.slug = $2
        "#,
    )
    .bind(tenant_slug)
    .bind(board_slug)
    .fetch_optional(pool)
    .await
}

pub async fn list_admin_boards(
    pool: &DbPool,
    tenant_id: Uuid,
) -> Result<Vec<BoardDetailDto>, sqlx::Error> {
    sqlx::query_as::<_, BoardDetailDto>(
        r#"
        SELECT id, slug, name, description, board_type, is_private, is_enabled, icon_url, background_color, dashboard_sections
        FROM boards
        WHERE tenant_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn create_board(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    slug: &str,
    name: &str,
    description: Option<&str>,
    board_type: &str,
    is_private: bool,
    icon_url: Option<&str>,
    background_color: Option<&str>,
    dashboard_sections: &[String],
) -> Result<BoardDetailDto, sqlx::Error> {
    sqlx::query_as::<_, BoardDetailDto>(
        r#"
        INSERT INTO boards (tenant_id, slug, name, description, board_type, is_private, icon_url, background_color, dashboard_sections, is_enabled)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, FALSE)
        RETURNING id, slug, name, description, board_type, is_private, is_enabled, icon_url, background_color, dashboard_sections
        "#,
    )
    .bind(tenant_id)
    .bind(slug)
    .bind(name)
    .bind(description)
    .bind(board_type)
    .bind(is_private)
    .bind(icon_url)
    .bind(background_color)
    .bind(dashboard_sections)
    .fetch_one(&mut **tx)
    .await
}

pub struct PreviousBoard {
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub board_type: String,
    pub is_private: bool,
    pub is_enabled: bool,
    pub is_default: bool,
    pub background_color: Option<String>,
}

pub async fn get_board_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    board_id: Uuid,
) -> Result<Option<PreviousBoard>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT tenant_id, slug, name, description, board_type, is_private, is_enabled, is_default, background_color FROM boards WHERE id = $1",
    )
    .bind(board_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row.map(|row| PreviousBoard {
        tenant_id: row.get("tenant_id"),
        slug: row.get("slug"),
        name: row.get("name"),
        description: row.get("description"),
        board_type: row.get("board_type"),
        is_private: row.get("is_private"),
        is_enabled: row.get("is_enabled"),
        is_default: row.get("is_default"),
        background_color: row.get("background_color"),
    }))
}

#[allow(clippy::too_many_arguments)]
pub async fn update_board(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    board_id: Uuid,
    name: &str,
    description: Option<&str>,
    board_type: &str,
    is_private: bool,
    icon_url: Option<&str>,
    background_color: Option<&str>,
    dashboard_sections: &[String],
    is_enabled: Option<bool>,
) -> Result<BoardDetailDto, sqlx::Error> {
    sqlx::query_as::<_, BoardDetailDto>(
        r#"
        UPDATE boards
        SET name = $1,
            description = $2,
            board_type = $3,
            is_private = $4,
            icon_url = $5,
            background_color = $6,
            dashboard_sections = $7,
            is_enabled = COALESCE($9, is_enabled),
            updated_at = NOW()
        WHERE id = $8
        RETURNING id, slug, name, description, board_type, is_private, is_enabled, icon_url, background_color, dashboard_sections
        "#,
    )
    .bind(name)
    .bind(description)
    .bind(board_type)
    .bind(is_private)
    .bind(icon_url)
    .bind(background_color)
    .bind(dashboard_sections)
    .bind(board_id)
    .bind(is_enabled)
    .fetch_one(&mut **tx)
    .await
}

pub async fn delete_board(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    board_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM boards WHERE id = $1")
        .bind(board_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn disable_default_board_for_tenant(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tenants SET default_board_enabled = FALSE, updated_at = NOW() WHERE id = $1",
    )
    .bind(tenant_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn get_board_tenant(pool: &DbPool, board_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query!("SELECT tenant_id FROM boards WHERE id = $1", board_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.tenant_id))
}

pub async fn get_tenant_id_by_slug(pool: &DbPool, slug: &str) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query!("SELECT id FROM tenants WHERE slug = $1", slug)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.id))
}

pub async fn get_tenant_id_by_slug_required(
    pool: &DbPool,
    slug: &str,
) -> Result<Uuid, sqlx::Error> {
    let row = sqlx::query!("SELECT id FROM tenants WHERE slug = $1", slug)
        .fetch_one(pool)
        .await?;
    Ok(row.id)
}

use std::collections::HashMap;

pub struct BoardSummaryData {
    pub slug: String,
    pub name: String,
    pub total_posts: i64,
    pub posts_by_status: HashMap<String, i64>,
    pub total_votes: i64,
    pub total_comments: i64,
}

pub async fn get_board_summary(
    pool: &DbPool,
    board_id: Uuid,
) -> Result<Option<BoardSummaryData>, sqlx::Error> {
    let board_info = sqlx::query!("SELECT slug, name FROM boards WHERE id = $1", board_id)
        .fetch_optional(pool)
        .await?;

    let board_info = match board_info {
        Some(b) => b,
        None => return Ok(None),
    };

    let total_posts = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM posts WHERE board_id = $1 AND is_hidden = false AND deleted_at IS NULL",
    )
    .bind(board_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let status_rows = sqlx::query!(
        "SELECT status, count(*)::bigint AS cnt FROM posts WHERE board_id = $1 AND is_hidden = false AND deleted_at IS NULL GROUP BY status",
        board_id
    )
    .fetch_all(pool)
    .await?;

    let posts_by_status: HashMap<String, i64> = status_rows
        .into_iter()
        .map(|row| (row.status, row.cnt.unwrap_or(0)))
        .collect();

    let total_votes = sqlx::query_scalar::<_, i64>(
        "SELECT coalesce(sum(vote_count), 0) FROM posts WHERE board_id = $1",
    )
    .bind(board_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let total_comments = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM comments c JOIN posts p ON c.post_id = p.id WHERE p.board_id = $1 AND c.is_hidden = false",
    )
    .bind(board_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    Ok(Some(BoardSummaryData {
        slug: board_info.slug,
        name: board_info.name,
        total_posts,
        posts_by_status,
        total_votes,
        total_comments,
    }))
}
