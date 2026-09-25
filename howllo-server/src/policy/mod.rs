use actix_web::{get, put, web, HttpRequest, HttpResponse, Responder};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::net::IpAddr;
use uuid::Uuid;

use crate::auth::{local_admin, require_permission, AuthenticatedUser};
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::errors::AppError;
use crate::repositories::membership_repository;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicyLimits {
    pub posts_per_hour: i32,
    pub posts_per_day: i32,
    pub comments_per_hour: i32,
    pub comments_per_day: i32,
    pub board_posts_per_10m: i32,
    pub board_comments_per_10m: i32,
    pub storage_mb: i32,
}

impl Default for PolicyLimits {
    fn default() -> Self {
        Self {
            posts_per_hour: 6,
            posts_per_day: 30,
            comments_per_hour: 60,
            comments_per_day: 300,
            board_posts_per_10m: 60,
            board_comments_per_10m: 300,
            storage_mb: 500,
        }
    }
}

fn default_caps() -> PolicyLimits {
    PolicyLimits {
        posts_per_hour: 100,
        posts_per_day: 500,
        comments_per_hour: 300,
        comments_per_day: 2000,
        board_posts_per_10m: 500,
        board_comments_per_10m: 1000,
        storage_mb: 10240,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicyOverrides {
    pub posts_per_hour: Option<i32>,
    pub posts_per_day: Option<i32>,
    pub comments_per_hour: Option<i32>,
    pub comments_per_day: Option<i32>,
    pub board_posts_per_10m: Option<i32>,
    pub board_comments_per_10m: Option<i32>,
    pub storage_mb: Option<i32>,
    pub ip_allowlist: Vec<String>,
    pub ip_blocklist: Vec<String>,
    pub blocked_countries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformPolicy {
    pub defaults: PolicyLimits,
    pub caps: PolicyLimits,
}

impl Default for PlatformPolicy {
    fn default() -> Self {
        Self {
            defaults: PolicyLimits::default(),
            caps: default_caps(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WorkspacePolicyView {
    pub effective: PolicyLimits,
    pub defaults: PolicyLimits,
    pub caps: PolicyLimits,
    pub overrides: PolicyOverrides,
    pub storage_cap_mb: Option<i32>,
    pub storage_used_bytes: i64,
}

fn fields(p: &PolicyLimits) -> [i32; 7] {
    [
        p.posts_per_hour,
        p.posts_per_day,
        p.comments_per_hour,
        p.comments_per_day,
        p.board_posts_per_10m,
        p.board_comments_per_10m,
        p.storage_mb,
    ]
}

fn validate_platform(policy: &PlatformPolicy) -> Result<(), AppError> {
    for (default, cap) in fields(&policy.defaults)
        .into_iter()
        .zip(fields(&policy.caps))
    {
        if default < 1 || cap < default || cap > 102400 {
            return Err(AppError::Validation(
                "Every default must be positive and at or below its platform cap (maximum 102400)."
                    .into(),
            ));
        }
    }
    if policy.defaults.posts_per_day < policy.defaults.posts_per_hour
        || policy.defaults.comments_per_day < policy.defaults.comments_per_hour
        || policy.caps.posts_per_day < policy.caps.posts_per_hour
        || policy.caps.comments_per_day < policy.caps.comments_per_hour
    {
        return Err(AppError::Validation(
            "Daily limits must be at least their hourly limits.".into(),
        ));
    }
    Ok(())
}

fn validate_overrides(input: &PolicyOverrides, caps: &PolicyLimits) -> Result<(), AppError> {
    let requested = [
        input.posts_per_hour,
        input.posts_per_day,
        input.comments_per_hour,
        input.comments_per_day,
        input.board_posts_per_10m,
        input.board_comments_per_10m,
        input.storage_mb,
    ];
    for (value, cap) in requested.into_iter().zip(fields(caps)) {
        if value.is_some_and(|n| n < 1 || n > cap) {
            return Err(AppError::Validation(format!(
                "Limit must be between 1 and {cap}."
            )));
        }
    }
    if input.ip_allowlist.len() > 100
        || input.ip_blocklist.len() > 100
        || input.blocked_countries.len() > 100
    {
        return Err(AppError::Validation(
            "Too many IP or country rules (maximum 100 per list).".into(),
        ));
    }
    for value in input.ip_allowlist.iter().chain(&input.ip_blocklist) {
        if value.parse::<IpNet>().is_err() && value.parse::<IpAddr>().is_err() {
            return Err(AppError::Validation(format!(
                "Invalid IP address or CIDR: {value}"
            )));
        }
    }
    for country in &input.blocked_countries {
        if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(AppError::Validation(format!(
                "Invalid two-letter country code: {country}"
            )));
        }
    }
    Ok(())
}

fn resolve(
    defaults: &PolicyLimits,
    caps: &PolicyLimits,
    o: &PolicyOverrides,
    storage_cap: Option<i32>,
) -> PolicyLimits {
    let bounded = |v: Option<i32>, default: i32, cap: i32| v.unwrap_or(default).min(cap);
    PolicyLimits {
        posts_per_hour: bounded(
            o.posts_per_hour,
            defaults.posts_per_hour,
            caps.posts_per_hour,
        ),
        posts_per_day: bounded(o.posts_per_day, defaults.posts_per_day, caps.posts_per_day),
        comments_per_hour: bounded(
            o.comments_per_hour,
            defaults.comments_per_hour,
            caps.comments_per_hour,
        ),
        comments_per_day: bounded(
            o.comments_per_day,
            defaults.comments_per_day,
            caps.comments_per_day,
        ),
        board_posts_per_10m: bounded(
            o.board_posts_per_10m,
            defaults.board_posts_per_10m,
            caps.board_posts_per_10m,
        ),
        board_comments_per_10m: bounded(
            o.board_comments_per_10m,
            defaults.board_comments_per_10m,
            caps.board_comments_per_10m,
        ),
        storage_mb: bounded(
            o.storage_mb,
            defaults.storage_mb,
            caps.storage_mb.min(storage_cap.unwrap_or(caps.storage_mb)),
        ),
    }
}

pub async fn platform_policy(pool: &PgPool) -> Result<PlatformPolicy, AppError> {
    let raw: Option<String> =
        sqlx::query_scalar("SELECT value FROM system_settings WHERE key = 'workspace_policy'")
            .fetch_optional(pool)
            .await
            .map_err(|_| AppError::InternalServerError)?;
    match raw {
        Some(raw) => serde_json::from_str(&raw).map_err(|_| AppError::InternalServerError),
        None => Ok(PlatformPolicy::default()),
    }
}

pub async fn workspace_policy(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<WorkspacePolicyView, AppError> {
    let platform = platform_policy(pool).await?;
    let row = sqlx::query(
        "SELECT overrides, storage_cap_mb FROM workspace_policies WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;
    let overrides: PolicyOverrides = row
        .as_ref()
        .map(|r| serde_json::from_value(r.get("overrides")))
        .transpose()
        .map_err(|_| AppError::InternalServerError)?
        .unwrap_or_default();
    let storage_cap_mb: Option<i32> = row.as_ref().and_then(|r| r.get("storage_cap_mb"));
    let used: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(size_bytes), 0)::bigint FROM workspace_assets WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;
    let effective = resolve(
        &platform.defaults,
        &platform.caps,
        &overrides,
        storage_cap_mb,
    );
    Ok(WorkspacePolicyView {
        effective,
        defaults: platform.defaults,
        caps: platform.caps,
        overrides,
        storage_cap_mb,
        storage_used_bytes: used,
    })
}

pub async fn enforce_ip_policy(
    req: &HttpRequest,
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<(), AppError> {
    let view = workspace_policy(pool, tenant_id).await?;
    if view.overrides.ip_allowlist.is_empty()
        && view.overrides.ip_blocklist.is_empty()
        && view.overrides.blocked_countries.is_empty()
    {
        return Ok(());
    }
    let peer = req.peer_addr().map(|addr| addr.ip());
    let forwarded = if peer.is_some_and(|ip| ip.is_loopback()) {
        req.headers()
            .get("cf-connecting-ip")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<IpAddr>().ok())
    } else {
        None
    };
    let ip = forwarded.or(peer).ok_or(AppError::Forbidden)?;
    let matches = |list: &[String]| {
        list.iter().any(|v| {
            v.parse::<IpNet>()
                .map(|net| net.contains(&ip))
                .or_else(|_| v.parse::<IpAddr>().map(|addr| addr == ip))
                .unwrap_or(false)
        })
    };
    if matches(&view.overrides.ip_blocklist)
        || (!view.overrides.ip_allowlist.is_empty() && !matches(&view.overrides.ip_allowlist))
    {
        return Err(AppError::Forbidden);
    }
    if peer.is_some_and(|addr| addr.is_loopback()) {
        if let Some(country) = req
            .headers()
            .get("cf-ipcountry")
            .and_then(|h| h.to_str().ok())
        {
            if view
                .overrides
                .blocked_countries
                .iter()
                .any(|v| v.eq_ignore_ascii_case(country))
            {
                return Err(AppError::Forbidden);
            }
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct TenantQuery {
    pub tenant_slug: String,
}

#[get("/api/admin/platform/policy")]
pub async fn get_platform_policy(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    if !local_admin::is_local_admin_user(&auth.0) {
        return Err(AppError::Forbidden);
    }
    Ok(HttpResponse::Ok().json(platform_policy(pool.get_ref()).await?))
}

#[put("/api/admin/platform/policy")]
pub async fn put_platform_policy(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<PlatformPolicy>,
) -> Result<impl Responder, AppError> {
    if !local_admin::is_local_admin_user(&auth.0) {
        return Err(AppError::Forbidden);
    }
    validate_platform(&body)?;
    let raw = serde_json::to_string(&body.0).map_err(|_| AppError::InternalServerError)?;
    sqlx::query("INSERT INTO system_settings (key, value) VALUES ('workspace_policy', $1) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now()")
        .bind(raw).execute(pool.get_ref()).await.map_err(|_| AppError::InternalServerError)?;
    Ok(HttpResponse::Ok().json(body.0))
}

#[get("/api/admin/workspace-policy")]
pub async fn get_workspace_policy(
    pool: web::Data<DbPool>,
    query: web::Query<TenantQuery>,
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
    crate::platform::ensure_workspace_assets_reconciled(pool.get_ref(), tenant_id).await?;
    Ok(HttpResponse::Ok().json(workspace_policy(pool.get_ref(), tenant_id).await?))
}

#[put("/api/admin/workspace-policy")]
pub async fn put_workspace_policy(
    pool: web::Data<DbPool>,
    query: web::Query<TenantQuery>,
    auth: AuthenticatedUser,
    body: web::Json<PolicyOverrides>,
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
    let platform = platform_policy(pool.get_ref()).await?;
    let cap: Option<i32> =
        sqlx::query_scalar("SELECT storage_cap_mb FROM workspace_policies WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_optional(pool.get_ref())
            .await
            .map_err(|_| AppError::InternalServerError)?
            .flatten();
    let mut caps = platform.caps;
    caps.storage_mb = caps.storage_mb.min(cap.unwrap_or(caps.storage_mb));
    validate_overrides(&body, &caps)?;
    let effective = resolve(&platform.defaults, &caps, &body, cap);
    if effective.posts_per_day < effective.posts_per_hour
        || effective.comments_per_day < effective.comments_per_hour
    {
        return Err(AppError::Validation(
            "Daily limits must be at least their hourly limits.".into(),
        ));
    }
    sqlx::query("INSERT INTO workspace_policies (tenant_id, overrides) VALUES ($1, $2) ON CONFLICT (tenant_id) DO UPDATE SET overrides = EXCLUDED.overrides")
        .bind(tenant_id).bind(serde_json::to_value(&body.0).map_err(|_| AppError::InternalServerError)?)
        .execute(pool.get_ref()).await.map_err(|_| AppError::InternalServerError)?;
    Ok(HttpResponse::Ok().json(workspace_policy(pool.get_ref(), tenant_id).await?))
}

#[derive(Deserialize)]
pub struct StorageCapUpdate {
    pub storage_cap_mb: Option<i32>,
}

#[get("/api/admin/platform/workspaces/{tenant_slug}/policy")]
pub async fn get_platform_workspace_policy(
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    if !local_admin::is_local_admin_user(&auth.0) {
        return Err(AppError::Forbidden);
    }
    let tenant_id =
        membership_repository::resolve_tenant_id(pool.get_ref(), &path.into_inner()).await?;
    crate::platform::ensure_workspace_assets_reconciled(pool.get_ref(), tenant_id).await?;
    Ok(HttpResponse::Ok().json(workspace_policy(pool.get_ref(), tenant_id).await?))
}

#[put("/api/admin/platform/workspaces/{tenant_slug}/storage-cap")]
pub async fn put_workspace_storage_cap(
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    auth: AuthenticatedUser,
    body: web::Json<StorageCapUpdate>,
) -> Result<impl Responder, AppError> {
    if !local_admin::is_local_admin_user(&auth.0) {
        return Err(AppError::Forbidden);
    }
    let tenant_id =
        membership_repository::resolve_tenant_id(pool.get_ref(), &path.into_inner()).await?;
    let platform = platform_policy(pool.get_ref()).await?;
    if body
        .storage_cap_mb
        .is_some_and(|v| v < 1 || v > platform.caps.storage_mb)
    {
        return Err(AppError::Validation(format!(
            "Storage cap must be between 1 and {} MB.",
            platform.caps.storage_mb
        )));
    }
    sqlx::query("INSERT INTO workspace_policies (tenant_id, storage_cap_mb) VALUES ($1, $2) ON CONFLICT (tenant_id) DO UPDATE SET storage_cap_mb = EXCLUDED.storage_cap_mb")
        .bind(tenant_id).bind(body.storage_cap_mb).execute(pool.get_ref()).await.map_err(|_| AppError::InternalServerError)?;
    Ok(HttpResponse::Ok().json(workspace_policy(pool.get_ref(), tenant_id).await?))
}

#[cfg(test)]
mod tests {
    use crate::db;
    use crate::http::test_support::{
        bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
    };
    use crate::startup;
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;

    #[actix_web::test]
    async fn tenant_override_and_ip_block_apply_to_writes() {
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
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let member = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );
        let update = test::TestRequest::put()
            .uri(&format!(
                "/api/admin/workspace-policy?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", admin))
            .set_json(
                json!({"posts_per_hour": 12, "posts_per_day": 50, "storage_mb": 100,
                "ip_blocklist": ["203.0.113.0/24"], "blocked_countries": ["XX"]}),
            )
            .to_request();
        let response = test::call_service(&app, update).await;
        assert_eq!(response.status(), StatusCode::OK);
        let view = read_json(response).await;
        assert_eq!(view["effective"]["posts_per_hour"], 12);
        assert_eq!(view["effective"]["storage_mb"], 100);

        let blocked = test::TestRequest::post()
            .uri(&format!("/api/boards/{}/posts", seed.board_slug))
            .peer_addr("203.0.113.7:12345".parse().unwrap())
            .insert_header(("Authorization", member))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "title": "Blocked idea", "body": "Blocked"}))
            .to_request();
        assert_eq!(
            test::call_service(&app, blocked).await.status(),
            StatusCode::FORBIDDEN
        );
    }

    #[actix_web::test]
    async fn platform_defaults_must_fit_caps() {
        let mut policy = super::PlatformPolicy::default();
        policy.defaults.posts_per_hour = policy.caps.posts_per_hour + 1;
        assert!(super::validate_platform(&policy).is_err());
    }

    #[actix_web::test]
    async fn workspace_storage_cap_wins_over_tenant_override() {
        let platform = super::PlatformPolicy::default();
        let overrides = super::PolicyOverrides {
            storage_mb: Some(1000),
            ..Default::default()
        };
        let effective = super::resolve(&platform.defaults, &platform.caps, &overrides, Some(200));
        assert_eq!(effective.storage_mb, 200);
    }
}
