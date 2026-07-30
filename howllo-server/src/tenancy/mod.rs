use actix_web::{get, patch, web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::auth::{require_permission, AuthenticatedUser};
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::errors::AppError;
use crate::repositories::board_repository;
use crate::repositories::{membership_repository, tenant_branding_repository};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct BootstrapTenantDto {
    pub default_tenant_slug: String,
    pub default_tenant_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TenantBrandingDto {
    pub tenant_slug: String,
    pub tenant_name: String,
    pub site_name: String,
    pub logo_url: Option<String>,
    pub accent_color: Option<String>,
    pub background_color: Option<String>,
    pub show_powered_by: bool,
    pub show_roadmap: bool,
}

#[derive(Debug, Deserialize)]
pub struct TenantBrandingQuery {
    pub tenant_slug: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantBrandingRequest {
    pub site_name: Option<String>,
    pub logo_url: Option<String>,
    pub accent_color: Option<String>,
    pub background_color: Option<String>,
    pub show_powered_by: bool,
    pub show_roadmap: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceAuthConfigDto {
    pub tenant_slug: String,
    pub provider: String,
    pub rooiam_workspace_id: Option<String>,
    pub rooiam_client_id: Option<String>,
    pub rooiam_widget_base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceAuthConfigRequest {
    pub provider: Option<String>,
    pub rooiam_workspace_id: Option<String>,
    pub rooiam_client_id: Option<String>,
    pub rooiam_widget_base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteWorkspaceRequest {
    /// Must exactly equal the target slug — the typed confirmation. The server
    /// enforces it so a delete can't fire from a stray/forged request.
    pub confirm_slug: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct AdminTenantSummaryDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub board_count: i64,
    pub member_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn ensure_bootstrap_tenant(pool: &DbPool) -> Result<BootstrapTenantDto, sqlx::Error> {
    if let Some(existing) = get_first_tenant(pool).await? {
        board_repository::ensure_default_board_for_tenant_slug(pool, &existing.default_tenant_slug)
            .await?;
        return Ok(existing);
    }

    let slug = format!("howllo-demo-{}", &Uuid::new_v4().simple().to_string()[..6]);
    let tenant_id = Uuid::new_v4();

    sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
        .bind(tenant_id)
        .bind(&slug)
        .bind("Howllo Demo")
        .execute(pool)
        .await?;

    board_repository::ensure_default_board_for_tenant_slug(pool, &slug).await?;

    Ok(BootstrapTenantDto {
        default_tenant_slug: slug,
        default_tenant_name: "Howllo Demo".to_string(),
    })
}

async fn get_first_tenant(pool: &DbPool) -> Result<Option<BootstrapTenantDto>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT slug, name
        FROM tenants
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| BootstrapTenantDto {
        default_tenant_slug: row.get("slug"),
        default_tenant_name: row.get("name"),
    }))
}

#[get("/api/bootstrap")]
pub async fn get_bootstrap_tenant(pool: web::Data<DbPool>) -> Result<impl Responder, AppError> {
    let tenant = ensure_bootstrap_tenant(pool.get_ref())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "error ensuring bootstrap tenant");
            AppError::InternalServerError
        })?;

    Ok(HttpResponse::Ok().json(tenant))
}

#[get("/api/admin/tenants")]
pub async fn list_admin_tenants(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    // Return only the workspaces this user can reach: ones they staff directly,
    // plus every workspace in an account they own/admin. The local (system) admin
    // holds a membership on every workspace, so it still sees them all.
    // Untyped query so no sqlx offline-cache regeneration is needed.
    let tenants = sqlx::query_as::<_, AdminTenantSummaryDto>(
        r#"
        SELECT
            t.id,
            t.slug,
            t.name,
            COUNT(DISTINCT b.id)::BIGINT AS board_count,
            COUNT(DISTINCT m.user_id)::BIGINT AS member_count,
            t.created_at,
            t.updated_at
        FROM tenants t
        LEFT JOIN boards b ON b.tenant_id = t.id
        LEFT JOIN memberships m ON m.tenant_id = t.id
        WHERE t.id IN (SELECT tenant_id FROM memberships WHERE user_id = $1)
           OR t.account_id IN (
                SELECT account_id FROM account_memberships
                WHERE user_id = $1 AND role IN ('owner', 'admin')
           )
        GROUP BY t.id, t.slug, t.name, t.created_at, t.updated_at
        ORDER BY t.created_at ASC
        "#,
    )
    .bind(auth.0.id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "error listing admin tenants");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Ok().json(tenants))
}

#[actix_web::post("/api/admin/tenants")]
pub async fn create_admin_tenant(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<CreateWorkspaceRequest>,
) -> Result<impl Responder, AppError> {
    // Any authenticated user can create a workspace; the first one also mints
    // their account and makes them its owner (SaaS sign-up flow).
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation(
            "workspace name is required".to_string(),
        ));
    }

    let requested_base = body.slug.as_deref().unwrap_or(name);
    let base_slug = slugify_workspace_base(requested_base)?;
    let final_slug = format!("{base_slug}-{}", &Uuid::new_v4().simple().to_string()[..6]);

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(error = %error, "error starting workspace creation transaction");
        AppError::InternalServerError
    })?;

    let row = sqlx::query!(
        r#"
        INSERT INTO tenants (slug, name)
        VALUES ($1, $2)
        RETURNING id, slug, name, created_at, updated_at
        "#,
        final_slug,
        name,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, slug = final_slug, "error creating workspace");
        AppError::InternalServerError
    })?;

    membership_repository::upsert_membership(&mut tx, row.id, auth.0.id, "owner")
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %row.id, user_id = %auth.0.id, "error assigning workspace owner");
            AppError::InternalServerError
        })?;

    // Group the new workspace under the creator's account (create one on first
    // workspace). See docs/TENANCY.md.
    let account_id = crate::repositories::account_repository::find_or_create_account_for_owner(
        &mut tx,
        auth.0.id,
        name,
    )
    .await
    .map_err(|error| {
        tracing::error!(error = %error, user_id = %auth.0.id, "error resolving account for workspace");
        AppError::InternalServerError
    })?;

    sqlx::query("UPDATE tenants SET account_id = $1 WHERE id = $2")
        .bind(account_id)
        .bind(row.id)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %row.id, account_id = %account_id, "error linking workspace to account");
            AppError::InternalServerError
        })?;

    tx.commit().await.map_err(|error| {
        tracing::error!(error = %error, tenant_id = %row.id, "error committing workspace creation");
        AppError::InternalServerError
    })?;

    board_repository::ensure_default_board_for_tenant_slug(pool.get_ref(), &row.slug)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_slug = row.slug.as_str(), "error bootstrapping workspace boards");
            AppError::InternalServerError
        })?;

    Ok(HttpResponse::Created().json(AdminTenantSummaryDto {
        id: row.id,
        slug: row.slug,
        name: row.name,
        board_count: 3,
        member_count: 1,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

#[actix_web::delete("/api/admin/tenants/{slug}")]
pub async fn delete_admin_tenant(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<DeleteWorkspaceRequest>,
) -> Result<impl Responder, AppError> {
    let slug = path.into_inner();

    // Typed confirmation must match the slug exactly.
    if body.confirm_slug.trim() != slug {
        return Err(AppError::Validation(
            "Confirmation text does not match the workspace slug.".to_string(),
        ));
    }

    // Find the target and the oldest (default/bootstrap) tenant in one pass.
    let rows = sqlx::query!("SELECT id, slug FROM tenants ORDER BY created_at ASC")
        .fetch_all(pool.get_ref())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "error loading tenants for delete");
            AppError::InternalServerError
        })?;

    let target = rows
        .iter()
        .find(|r| r.slug == slug)
        .ok_or(AppError::NotFound)?;

    // Only a workspace owner (or account owner, via the authz shortcut) may delete it.
    crate::auth::require_owner(pool.get_ref(), target.id, auth.0.id).await?;

    // Never delete the default workspace (oldest) — the app bootstraps from it.
    if rows.first().map(|r| r.id) == Some(target.id) {
        return Err(AppError::Validation(
            "The default workspace cannot be deleted.".to_string(),
        ));
    }

    // Cascades to boards, posts, comments, memberships, audit, etc. via FK.
    sqlx::query!("DELETE FROM tenants WHERE id = $1", target.id)
        .execute(pool.get_ref())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_slug = slug.as_str(), "error deleting workspace");
            AppError::InternalServerError
        })?;

    tracing::warn!(
        tenant_slug = slug.as_str(),
        "workspace deleted by local admin"
    );
    Ok(HttpResponse::NoContent().finish())
}

#[get("/api/tenant-branding")]
pub async fn get_tenant_branding(
    pool: web::Data<DbPool>,
    query: web::Query<TenantBrandingQuery>,
) -> Result<impl Responder, AppError> {
    let branding = fetch_tenant_branding(pool.get_ref(), &query.tenant_slug).await?;
    Ok(HttpResponse::Ok().json(branding))
}

#[get("/api/workspace-auth")]
pub async fn get_workspace_auth_config(
    pool: web::Data<DbPool>,
    settings: web::Data<crate::config::Settings>,
    query: web::Query<TenantBrandingQuery>,
) -> Result<impl Responder, AppError> {
    let config =
        fetch_workspace_auth_config(pool.get_ref(), settings.get_ref(), &query.tenant_slug).await?;
    Ok(HttpResponse::Ok().json(config))
}

#[patch("/api/admin/tenant-branding")]
pub async fn update_tenant_branding(
    pool: web::Data<DbPool>,
    query: web::Query<TenantBrandingQuery>,
    body: web::Json<UpdateTenantBrandingRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id =
        membership_repository::resolve_tenant_id(pool.get_ref(), &query.tenant_slug).await?;
    require_permission(
        pool.get_ref(),
        tenant_id,
        auth.0.id,
        Permission::ManageSettings,
    )
    .await?;

    let site_name = normalize_optional_text(body.site_name.as_deref());
    let logo_url = normalize_optional_text(body.logo_url.as_deref());
    let accent_color = normalize_optional_text(body.accent_color.as_deref());
    let background_color = normalize_optional_text(body.background_color.as_deref());

    if let Some(ref value) = accent_color {
        validate_accent_color(value)?;
    }
    if let Some(ref value) = background_color {
        validate_color(value, "background_color")?;
    }

    tenant_branding_repository::upsert(
        pool.get_ref(),
        tenant_id,
        site_name.as_deref(),
        logo_url.as_deref(),
        accent_color.as_deref(),
        background_color.as_deref(),
        body.show_powered_by,
        body.show_roadmap,
    )
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, "error updating tenant branding");
        AppError::InternalServerError
    })?;

    let branding = fetch_tenant_branding(pool.get_ref(), &query.tenant_slug).await?;
    Ok(HttpResponse::Ok().json(branding))
}

#[patch("/api/admin/workspace-auth")]
pub async fn update_workspace_auth_config(
    pool: web::Data<DbPool>,
    settings: web::Data<crate::config::Settings>,
    query: web::Query<TenantBrandingQuery>,
    body: web::Json<UpdateWorkspaceAuthConfigRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id =
        membership_repository::resolve_tenant_id(pool.get_ref(), &query.tenant_slug).await?;
    require_permission(
        pool.get_ref(),
        tenant_id,
        auth.0.id,
        Permission::ManageSettings,
    )
    .await?;

    sqlx::query(
        r#"
        INSERT INTO workspace_auth_configs (
            tenant_id,
            provider,
            rooiam_workspace_id,
            rooiam_client_id,
            rooiam_widget_base_url
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (tenant_id)
        DO UPDATE SET
            provider = EXCLUDED.provider,
            rooiam_workspace_id = EXCLUDED.rooiam_workspace_id,
            rooiam_client_id = EXCLUDED.rooiam_client_id,
            rooiam_widget_base_url = EXCLUDED.rooiam_widget_base_url,
            updated_at = NOW()
        "#,
    )
    .bind(tenant_id)
    .bind(
        body.provider
            .as_deref()
            .unwrap_or("rooiam")
            .trim()
            .to_string(),
    )
    .bind(normalize_optional_text(body.rooiam_workspace_id.as_deref()))
    .bind(normalize_optional_text(body.rooiam_client_id.as_deref()))
    .bind(normalize_optional_text(body.rooiam_widget_base_url.as_deref()))
    .execute(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, "error updating workspace auth config");
        AppError::InternalServerError
    })?;

    let config =
        fetch_workspace_auth_config(pool.get_ref(), settings.get_ref(), &query.tenant_slug).await?;
    Ok(HttpResponse::Ok().json(config))
}

// --------------------------------------------------------------- SSO secret
// The end-user SSO shared secret is sensitive: only owners/admins may see it,
// and it is never returned by the public /api/workspace-auth endpoint.

async fn require_settings_for(
    pool: &DbPool,
    tenant_slug: &str,
    actor_user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, tenant_slug).await?;
    require_permission(pool, tenant_id, actor_user_id, Permission::ManageSettings).await?;
    Ok(tenant_id)
}

fn sso_config_json(secret: Option<String>) -> serde_json::Value {
    let secret = secret.filter(|value| !value.trim().is_empty());
    serde_json::json!({
        "enabled": secret.is_some(),
        "secret": secret,
        "session_path": "/api/auth/sso-session",
    })
}

#[get("/api/admin/sso-config")]
pub async fn get_sso_config(
    pool: web::Data<DbPool>,
    query: web::Query<TenantBrandingQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = require_settings_for(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    let secret: Option<String> =
        sqlx::query_scalar("SELECT sso_secret FROM workspace_auth_configs WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_optional(pool.get_ref())
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "error reading sso secret");
                AppError::InternalServerError
            })?
            .flatten();
    Ok(HttpResponse::Ok().json(sso_config_json(secret)))
}

#[actix_web::post("/api/admin/sso-config/regenerate")]
pub async fn regenerate_sso_secret(
    pool: web::Data<DbPool>,
    query: web::Query<TenantBrandingQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = require_settings_for(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    let secret = format!("sk_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO workspace_auth_configs (tenant_id, sso_secret)
        VALUES ($1, $2)
        ON CONFLICT (tenant_id) DO UPDATE SET sso_secret = EXCLUDED.sso_secret, updated_at = NOW()
        "#,
    )
    .bind(tenant_id)
    .bind(&secret)
    .execute(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "error regenerating sso secret");
        AppError::InternalServerError
    })?;
    Ok(HttpResponse::Ok().json(sso_config_json(Some(secret))))
}

#[actix_web::delete("/api/admin/sso-config")]
pub async fn disable_sso(
    pool: web::Data<DbPool>,
    query: web::Query<TenantBrandingQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = require_settings_for(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    sqlx::query("UPDATE workspace_auth_configs SET sso_secret = NULL, updated_at = NOW() WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(pool.get_ref())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "error disabling sso");
            AppError::InternalServerError
        })?;
    Ok(HttpResponse::Ok().json(sso_config_json(None)))
}

async fn fetch_tenant_branding(
    pool: &DbPool,
    tenant_slug: &str,
) -> Result<TenantBrandingDto, AppError> {
    let record = tenant_branding_repository::get_by_tenant_slug(pool, tenant_slug)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_slug = tenant_slug, "error fetching tenant branding");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    Ok(TenantBrandingDto {
        tenant_slug: record.tenant_slug,
        tenant_name: record.tenant_name.clone(),
        site_name: record.site_name.unwrap_or_else(|| record.tenant_name),
        logo_url: record.logo_url,
        accent_color: record.accent_color,
        background_color: record.background_color,
        show_powered_by: record.show_powered_by,
        show_roadmap: record.show_roadmap,
    })
}

async fn fetch_workspace_auth_config(
    pool: &DbPool,
    settings: &crate::config::Settings,
    tenant_slug: &str,
) -> Result<WorkspaceAuthConfigDto, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            t.slug AS tenant_slug,
            w.provider,
            w.rooiam_workspace_id,
            w.rooiam_client_id,
            w.rooiam_widget_base_url
        FROM tenants t
        LEFT JOIN workspace_auth_configs w ON w.tenant_id = t.id
        WHERE t.slug = $1
        "#,
    )
    .bind(tenant_slug)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_slug, "error fetching workspace auth config");
        AppError::InternalServerError
    })?
    .ok_or(AppError::NotFound)?;

    Ok(WorkspaceAuthConfigDto {
        tenant_slug: row.get("tenant_slug"),
        provider: row
            .get::<Option<String>, _>("provider")
            .unwrap_or_else(|| "rooiam".to_string()),
        rooiam_workspace_id: row
            .get::<Option<String>, _>("rooiam_workspace_id")
            .or_else(|| settings.rooiam_widget_workspace_id.clone()),
        rooiam_client_id: row
            .get::<Option<String>, _>("rooiam_client_id")
            .or_else(|| settings.rooiam_widget_client_id.clone()),
        rooiam_widget_base_url: row
            .get::<Option<String>, _>("rooiam_widget_base_url")
            .or_else(|| settings.rooiam_widget_base_url.clone()),
    })
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn validate_accent_color(value: &str) -> Result<(), AppError> {
    validate_color(value, "accent_color")
}

fn validate_color(value: &str, field: &str) -> Result<(), AppError> {
    let bytes = value.as_bytes();
    let is_valid = bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit());

    if is_valid {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "{field} must be a hex color like #f36949"
        )))
    }
}

fn slugify_workspace_base(value: &str) -> Result<String, AppError> {
    let mut slug = String::with_capacity(value.len());
    let mut prev_dash = false;

    for ch in value.trim().chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        return Err(AppError::Validation(
            "workspace slug must contain letters or numbers".to_string(),
        ));
    }

    Ok(slug)
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;

    use crate::db;
    use crate::http::test_support::{
        bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
    };
    use crate::startup;

    // Creating workspaces groups them under the creator's account: the first
    // one mints an account, the second reuses it. See docs/TENANCY.md.
    #[actix_web::test]
    async fn creating_workspaces_groups_them_under_one_account() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let _seed = seed_basic_tenant(&pool).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(crate::realtime::Hub::new()))
                .configure(startup::configure),
        )
        .await;

        // Only the local (system) admin can create workspaces today.
        let token = bearer_for(
            "local-admin",
            "admin@howllo.local",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let first_req = test::TestRequest::post()
            .uri("/api/admin/tenants")
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({ "name": "Acme Product" }))
            .to_request();
        let first = read_json(test::call_service(&app, first_req).await).await;

        let second_req = test::TestRequest::post()
            .uri("/api/admin/tenants")
            .insert_header(("Authorization", token))
            .set_json(json!({ "name": "Acme Support" }))
            .to_request();
        let second = read_json(test::call_service(&app, second_req).await).await;

        let first_slug = first.get("slug").and_then(|v| v.as_str()).unwrap();
        let second_slug = second.get("slug").and_then(|v| v.as_str()).unwrap();

        // Both workspaces are linked to accounts...
        let account_ids: Vec<uuid::Uuid> = sqlx::query_scalar(
            "SELECT account_id FROM tenants WHERE slug = ANY($1) AND account_id IS NOT NULL",
        )
        .bind(vec![first_slug.to_string(), second_slug.to_string()])
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(account_ids.len(), 2, "both new workspaces have an account");
        // ...and it is the SAME account (the creator's).
        assert_eq!(account_ids[0], account_ids[1]);

        // Exactly one account membership (owner) for the local admin.
        let account_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM account_memberships WHERE role = 'owner'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(account_count, 1);
    }

    // list_admin_tenants is scoped to the caller, and an account owner can reach
    // (and manage) every workspace in their account — even one they have no direct
    // membership on. See docs/TENANCY.md.
    #[actix_web::test]
    async fn account_owner_reaches_every_workspace_in_their_account() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(crate::realtime::Hub::new()))
                .configure(startup::configure),
        )
        .await;

        // A plain rooiam user (not the system admin) creates their first workspace.
        let founder = bearer_for(
            "founder-1",
            "founder@example.com",
            "Founder",
            &settings.rooiam_jwt_secret,
        );
        let create_req = test::TestRequest::post()
            .uri("/api/admin/tenants")
            .insert_header(("Authorization", founder.clone()))
            .set_json(json!({ "name": "Founder One" }))
            .to_request();
        let w1 = read_json(test::call_service(&app, create_req).await).await;
        let w1_slug = w1.get("slug").and_then(|v| v.as_str()).unwrap().to_string();

        // A second workspace is put in the founder's account WITHOUT a direct
        // membership for them.
        let account_id: uuid::Uuid =
            sqlx::query_scalar("SELECT account_id FROM tenants WHERE slug = $1")
                .bind(&w1_slug)
                .fetch_one(&pool)
                .await
                .unwrap();
        let w2_slug = format!("founder-two-{}", &uuid::Uuid::new_v4().simple().to_string()[..6]);
        sqlx::query("INSERT INTO tenants (slug, name, account_id) VALUES ($1, 'Founder Two', $2)")
            .bind(&w2_slug)
            .bind(account_id)
            .execute(&pool)
            .await
            .unwrap();

        // Scoping: founder sees BOTH their workspaces, not the seed's.
        let list_req = test::TestRequest::get()
            .uri("/api/admin/tenants")
            .insert_header(("Authorization", founder.clone()))
            .to_request();
        let list = read_json(test::call_service(&app, list_req).await).await;
        let slugs: Vec<String> = list
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.get("slug").and_then(|v| v.as_str()).map(String::from))
            .collect();
        assert!(slugs.contains(&w1_slug));
        assert!(slugs.contains(&w2_slug));
        assert!(!slugs.contains(&seed.tenant_slug));

        // Authz shortcut: founder (account owner, no direct membership on W2) can
        // manage W2's settings.
        let patch_req = test::TestRequest::patch()
            .uri(&format!("/api/admin/tenant-branding?tenant_slug={w2_slug}"))
            .insert_header(("Authorization", founder))
            .set_json(json!({
                "site_name": "Founder Two",
                "accent_color": "#f36949",
                "background_color": "#e8f4ff",
                "show_powered_by": true,
                "show_roadmap": true
            }))
            .to_request();
        assert_eq!(
            test::call_service(&app, patch_req).await.status(),
            StatusCode::OK
        );

        // A stranger from another account cannot.
        let stranger = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );
        let denied_req = test::TestRequest::patch()
            .uri(&format!("/api/admin/tenant-branding?tenant_slug={w2_slug}"))
            .insert_header(("Authorization", stranger))
            .set_json(json!({
                "site_name": "Nope",
                "accent_color": "#000000",
                "background_color": "#ffffff",
                "show_powered_by": true,
                "show_roadmap": true
            }))
            .to_request();
        assert_eq!(
            test::call_service(&app, denied_req).await.status(),
            StatusCode::FORBIDDEN
        );
    }

    #[actix_web::test]
    async fn admin_can_update_workspace_branding_background() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(crate::realtime::Hub::new()))
                .configure(startup::configure),
        )
        .await;

        let token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let update_request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/tenant-branding?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({
                "site_name": "Acme Feedback",
                "accent_color": "#f36949",
                "background_color": "#e8f4ff",
                "show_powered_by": false,
                "show_roadmap": false
            }))
            .to_request();
        let update_response = test::call_service(&app, update_request).await;
        assert_eq!(update_response.status(), StatusCode::OK);
        let update_body = read_json(update_response).await;
        assert_eq!(
            update_body
                .get("background_color")
                .and_then(|value| value.as_str()),
            Some("#e8f4ff")
        );

        let get_request = test::TestRequest::get()
            .uri(&format!(
                "/api/tenant-branding?tenant_slug={}",
                seed.tenant_slug
            ))
            .to_request();
        let get_response = test::call_service(&app, get_request).await;
        assert_eq!(get_response.status(), StatusCode::OK);
        let get_body = read_json(get_response).await;
        assert_eq!(
            get_body
                .get("background_color")
                .and_then(|value| value.as_str()),
            Some("#e8f4ff")
        );
        assert_eq!(
            get_body
                .get("show_roadmap")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
    }
}
