use sqlx::Row;
use uuid::Uuid;

use crate::db::DbPool;

#[derive(Debug, Clone)]
pub struct TenantBrandingRecord {
    pub is_published: bool,
    pub tenant_id: Uuid,
    pub tenant_slug: String,
    pub tenant_name: String,
    pub site_name: Option<String>,
    pub logo_url: Option<String>,
    pub accent_color: Option<String>,
    pub background_color: Option<String>,
    pub show_powered_by: bool,
    pub show_roadmap: bool,
    pub show_boards: bool,
    pub show_feed: bool,
    pub require_post_approval: bool,
    pub posts_per_hour: i32,
    pub comments_per_hour: i32,
    pub board_posts_per_10m: i32,
    pub board_comments_per_10m: i32,
}

pub async fn get_by_tenant_slug(
    pool: &DbPool,
    tenant_slug: &str,
) -> Result<Option<TenantBrandingRecord>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            t.id AS tenant_id,
            t.slug AS tenant_slug,
            t.name AS tenant_name,
            t.is_published,
            b.site_name,
            b.logo_url,
            b.accent_color,
            b.background_color,
            COALESCE(b.show_powered_by, TRUE) AS show_powered_by,
            COALESCE(b.show_roadmap, TRUE) AS show_roadmap,
            COALESCE(b.show_boards, TRUE) AS show_boards,
            COALESCE(b.show_feed, TRUE) AS show_feed,
            COALESCE(b.require_post_approval, FALSE) AS require_post_approval,
            COALESCE(b.posts_per_hour, 3) AS posts_per_hour,
            COALESCE(b.comments_per_hour, 15) AS comments_per_hour,
            COALESCE(b.board_posts_per_10m, 20) AS board_posts_per_10m,
            COALESCE(b.board_comments_per_10m, 60) AS board_comments_per_10m
        FROM tenants t
        LEFT JOIN tenant_branding b ON b.tenant_id = t.id
        WHERE t.slug = $1
        "#,
    )
    .bind(tenant_slug)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_branding_row))
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &DbPool,
    tenant_id: Uuid,
    site_name: Option<&str>,
    logo_url: Option<&str>,
    accent_color: Option<&str>,
    background_color: Option<&str>,
    show_powered_by: bool,
    show_roadmap: bool,
    show_boards: Option<bool>,
    show_feed: Option<bool>,
    require_post_approval: Option<bool>,
    posts_per_hour: Option<i32>,
    comments_per_hour: Option<i32>,
    board_posts_per_10m: Option<i32>,
    board_comments_per_10m: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO tenant_branding (
            tenant_id,
            site_name,
            logo_url,
            accent_color,
            background_color,
            show_powered_by,
            show_roadmap,
            show_boards,
            show_feed,
            require_post_approval,
            posts_per_hour,
            comments_per_hour,
            board_posts_per_10m,
            board_comments_per_10m
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, COALESCE($8, TRUE), COALESCE($9, TRUE), COALESCE($10, FALSE), COALESCE($11, 3), COALESCE($12, 15), COALESCE($13, 20), COALESCE($14, 60))
        ON CONFLICT (tenant_id)
        DO UPDATE SET
            site_name = EXCLUDED.site_name,
            logo_url = EXCLUDED.logo_url,
            accent_color = EXCLUDED.accent_color,
            background_color = EXCLUDED.background_color,
            show_powered_by = EXCLUDED.show_powered_by,
            show_roadmap = EXCLUDED.show_roadmap,
            show_boards = COALESCE($8, tenant_branding.show_boards),
            show_feed = COALESCE($9, tenant_branding.show_feed),
            require_post_approval = COALESCE($10, tenant_branding.require_post_approval),
            posts_per_hour = COALESCE($11, tenant_branding.posts_per_hour),
            comments_per_hour = COALESCE($12, tenant_branding.comments_per_hour),
            board_posts_per_10m = COALESCE($13, tenant_branding.board_posts_per_10m),
            board_comments_per_10m = COALESCE($14, tenant_branding.board_comments_per_10m),
            updated_at = NOW()
        "#,
    )
    .bind(tenant_id)
    .bind(site_name)
    .bind(logo_url)
    .bind(accent_color)
    .bind(background_color)
    .bind(show_powered_by)
    .bind(show_roadmap)
    .bind(show_boards)
    .bind(show_feed)
    .bind(require_post_approval)
    .bind(posts_per_hour)
    .bind(comments_per_hour)
    .bind(board_posts_per_10m)
    .bind(board_comments_per_10m)
    .execute(pool)
    .await?;

    Ok(())
}

fn map_branding_row(row: sqlx::postgres::PgRow) -> TenantBrandingRecord {
    TenantBrandingRecord {
        is_published: row.get("is_published"),
        tenant_id: row.get("tenant_id"),
        tenant_slug: row.get("tenant_slug"),
        tenant_name: row.get("tenant_name"),
        site_name: row.get("site_name"),
        logo_url: row.get("logo_url"),
        accent_color: row.get("accent_color"),
        background_color: row.get("background_color"),
        show_powered_by: row.get("show_powered_by"),
        show_roadmap: row.get("show_roadmap"),
        show_boards: row.get("show_boards"),
        show_feed: row.get("show_feed"),
        require_post_approval: row.get("require_post_approval"),
        posts_per_hour: row.get("posts_per_hour"),
        comments_per_hour: row.get("comments_per_hour"),
        board_posts_per_10m: row.get("board_posts_per_10m"),
        board_comments_per_10m: row.get("board_comments_per_10m"),
    }
}
