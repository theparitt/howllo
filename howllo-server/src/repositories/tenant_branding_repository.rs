use sqlx::Row;
use uuid::Uuid;

use crate::db::DbPool;

#[derive(Debug, Clone)]
pub struct TenantBrandingRecord {
    pub tenant_id: Uuid,
    pub tenant_slug: String,
    pub tenant_name: String,
    pub site_name: Option<String>,
    pub logo_url: Option<String>,
    pub accent_color: Option<String>,
    pub show_powered_by: bool,
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
            b.site_name,
            b.logo_url,
            b.accent_color,
            COALESCE(b.show_powered_by, TRUE) AS show_powered_by
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

pub async fn upsert(
    pool: &DbPool,
    tenant_id: Uuid,
    site_name: Option<&str>,
    logo_url: Option<&str>,
    accent_color: Option<&str>,
    show_powered_by: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO tenant_branding (
            tenant_id,
            site_name,
            logo_url,
            accent_color,
            show_powered_by
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (tenant_id)
        DO UPDATE SET
            site_name = EXCLUDED.site_name,
            logo_url = EXCLUDED.logo_url,
            accent_color = EXCLUDED.accent_color,
            show_powered_by = EXCLUDED.show_powered_by,
            updated_at = NOW()
        "#,
    )
    .bind(tenant_id)
    .bind(site_name)
    .bind(logo_url)
    .bind(accent_color)
    .bind(show_powered_by)
    .execute(pool)
    .await?;

    Ok(())
}

fn map_branding_row(row: sqlx::postgres::PgRow) -> TenantBrandingRecord {
    TenantBrandingRecord {
        tenant_id: row.get("tenant_id"),
        tenant_slug: row.get("tenant_slug"),
        tenant_name: row.get("tenant_name"),
        site_name: row.get("site_name"),
        logo_url: row.get("logo_url"),
        accent_color: row.get("accent_color"),
        show_powered_by: row.get("show_powered_by"),
    }
}
