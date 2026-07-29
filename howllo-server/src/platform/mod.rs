//! Platform-level settings endpoints (local-admin only).
//!
//! Storage: GET current effective config, POST test (Local/MinIO), POST save.
//! Database: POST a read-only connection check.
//!
//! These are guarded by `local_admin::is_local_admin_user` — same gate the
//! platform tenant endpoints use.

use actix_web::{get, post, web, HttpResponse, Responder};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use uuid::Uuid;

use crate::auth::{local_admin, AuthenticatedUser};
use crate::config::Settings;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::storage::{
    load_platform_storage_config, save_platform_storage_config, test_local_storage,
    test_minio_storage, PlatformStorageConfigUpdate, StorageBackend,
};

fn ensure_platform_admin(auth: &AuthenticatedUser) -> Result<(), AppError> {
    if local_admin::is_local_admin_user(&auth.0) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

// ── Image upload (any authenticated user — for post screenshots) ───────────────

#[derive(Deserialize)]
pub struct UploadRequest {
    /// Original filename (used only to pick an extension).
    pub filename: String,
    /// MIME type, e.g. "image/png". Must be an image.
    pub content_type: String,
    /// Base64-encoded file bytes (no data: URL prefix).
    pub data: String,
}

#[derive(Serialize)]
pub struct UploadResponse {
    pub url: String,
}

const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024; // 8 MB

#[post("/api/uploads")]
pub async fn upload_image(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<UploadRequest>,
) -> Result<impl Responder, AppError> {
    let _ = auth; // any authenticated user may upload
    use base64::Engine;

    let content_type = body.content_type.trim().to_lowercase();
    if !content_type.starts_with("image/") {
        return Err(AppError::Validation(
            "Only image uploads are allowed.".into(),
        ));
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(body.data.trim())
        .map_err(|_| AppError::Validation("Invalid base64 image data.".into()))?;
    if bytes.is_empty() {
        return Err(AppError::Validation("Empty upload.".into()));
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(AppError::Validation("Image exceeds the 8 MB limit.".into()));
    }

    let ext = match content_type.as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    };
    let relative = format!("posts/{}.{}", uuid::Uuid::new_v4(), ext);

    let url = crate::storage::store_public_asset(pool.get_ref(), &relative, &bytes, &content_type)
        .await?;

    Ok(HttpResponse::Ok().json(UploadResponse { url }))
}

// ── Local file serving ─────────────────────────────────────────────────────────
// When the storage backend is Local, uploaded files live under the storage root
// and are served from /uploads/*. (With MinIO, the browser hits MinIO directly.)

#[get("/uploads/{path:.*}")]
pub async fn serve_upload(
    pool: web::Data<DbPool>,
    path: web::Path<String>,
) -> Result<impl Responder, AppError> {
    let rel = path.into_inner();
    // Reject path traversal.
    if rel.contains("..") {
        return Err(AppError::NotFound);
    }
    let cfg = load_platform_storage_config(pool.get_ref()).await?;
    let absolute = format!(
        "{}/{}",
        cfg.local_path.trim_end_matches('/'),
        rel.trim_start_matches('/')
    );
    let bytes = std::fs::read(&absolute).map_err(|_| AppError::NotFound)?;
    let content_type = match absolute.rsplit('.').next() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    };
    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .insert_header(("cache-control", "public, max-age=31536000, immutable"))
        .body(bytes))
}

// ── Storage ────────────────────────────────────────────────────────────────────

#[get("/api/admin/platform/storage")]
pub async fn get_storage_config(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    ensure_platform_admin(&auth)?;
    let cfg = load_platform_storage_config(pool.get_ref()).await?;
    Ok(HttpResponse::Ok().json(cfg))
}

#[post("/api/admin/platform/storage")]
pub async fn save_storage_config(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<PlatformStorageConfigUpdate>,
) -> Result<impl Responder, AppError> {
    ensure_platform_admin(&auth)?;
    let cfg = save_platform_storage_config(pool.get_ref(), &body).await?;
    tracing::info!(
        backend = cfg.backend.as_str(),
        "platform storage config saved"
    );
    Ok(HttpResponse::Ok().json(cfg))
}

#[derive(Deserialize)]
pub struct TestStorageRequest {
    pub backend: StorageBackend,
    pub local_path: Option<String>,
    pub public_base_url: Option<String>,
    pub minio_endpoint: Option<String>,
    pub minio_bucket: Option<String>,
    pub minio_access_key: Option<String>,
    pub minio_secret_key: Option<String>,
    pub minio_use_ssl: Option<bool>,
}

#[derive(Serialize)]
pub struct TestResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct WorkspaceStorageUsageItem {
    pub workspace_id: Uuid,
    pub workspace_slug: String,
    pub workspace_name: String,
    pub asset_count: usize,
    pub total_bytes: u64,
    pub logo_bytes: u64,
    pub attachment_bytes: u64,
}

#[derive(Serialize)]
pub struct WorkspaceStorageUsageResponse {
    pub backend: StorageBackend,
    pub total_assets: usize,
    pub total_bytes: u64,
    pub items: Vec<WorkspaceStorageUsageItem>,
}

#[derive(Serialize)]
pub struct PlatformStatusCheck {
    pub key: String,
    pub label: String,
    pub level: String,
    pub message: String,
    pub detail: Option<String>,
}

#[derive(Serialize)]
pub struct PlatformStatusResponse {
    pub overall: String,
    pub checked_at: chrono::DateTime<Utc>,
    pub checks: Vec<PlatformStatusCheck>,
}

// ── Build info + masked environment (debug badge) ──────────────────────────────

#[derive(Serialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Serialize)]
pub struct BuildInfoResponse {
    pub version: String,
    pub built_at: String,
    pub server_time: chrono::DateTime<Utc>,
    /// Effective env values with secrets masked. Local-admin only.
    pub env: Vec<EnvVar>,
}

/// Mask the password in a connection URL: postgres://user:****@host/db.
fn mask_url(value: &str) -> String {
    match url::Url::parse(value) {
        Ok(mut u) => {
            if u.password().is_some() {
                let _ = u.set_password(Some("****"));
            }
            u.to_string()
        }
        Err(_) => value.to_string(),
    }
}

/// Show that a secret is set without revealing it.
fn mask_secret(value: &str) -> String {
    if value.trim().is_empty() {
        "(not set)".to_string()
    } else {
        "•••• (set)".to_string()
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[get("/api/admin/platform/build")]
pub async fn get_build_info(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    ensure_platform_admin(&auth)?;

    // Effective storage config (DB override → env) so the badge reflects what's
    // actually in use, not just raw env.
    let storage = load_platform_storage_config(pool.get_ref()).await?;

    let env = vec![
        EnvVar {
            key: "HOWLLO_DATABASE_URL".into(),
            value: mask_url(&settings.database_url),
        },
        EnvVar {
            key: "HOWLLO_BIND_ADDRESS".into(),
            value: settings.bind_address.clone(),
        },
        EnvVar {
            key: "HOWLLO_JWT_SECRET".into(),
            value: mask_secret(&settings.rooiam_jwt_secret),
        },
        EnvVar {
            key: "HOWLLO_ADMIN_BOOTSTRAP_KEY".into(),
            value: mask_secret(settings.admin_bootstrap_key.as_deref().unwrap_or("")),
        },
        EnvVar {
            key: "HOWLLO_STORAGE_BACKEND".into(),
            value: storage.backend.as_str().to_string(),
        },
        EnvVar {
            key: "HOWLLO_STORAGE_PUBLIC_BASE_URL".into(),
            value: storage.public_base_url.clone(),
        },
        EnvVar {
            key: "HOWLLO_MINIO_ENDPOINT".into(),
            value: env_or("HOWLLO_MINIO_ENDPOINT", "(not set)"),
        },
        EnvVar {
            key: "HOWLLO_MINIO_BUCKET".into(),
            value: env_or("HOWLLO_MINIO_BUCKET", "(not set)"),
        },
        EnvVar {
            key: "HOWLLO_MINIO_USER".into(),
            value: env_or("HOWLLO_MINIO_USER", "(not set)"),
        },
        EnvVar {
            key: "HOWLLO_MINIO_PASSWORD".into(),
            value: mask_secret(&env_or("HOWLLO_MINIO_PASSWORD", "")),
        },
        EnvVar {
            key: "HOWLLO_ALLOWED_ORIGINS".into(),
            value: settings.allowed_origins.join(", "),
        },
        EnvVar {
            key: "HOWLLO_AI_ENABLED".into(),
            value: settings.ai.enabled.to_string(),
        },
    ];

    Ok(HttpResponse::Ok().json(BuildInfoResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        built_at: env!("HOWLLO_BUILT_AT").to_string(),
        server_time: Utc::now(),
        env,
    }))
}

#[post("/api/admin/platform/storage/test")]
pub async fn test_storage(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<TestStorageRequest>,
) -> Result<impl Responder, AppError> {
    ensure_platform_admin(&auth)?;

    match body.backend {
        StorageBackend::Local => {
            let path = body.local_path.as_deref().unwrap_or("").trim();
            match test_local_storage(path) {
                Ok(message) => Ok(HttpResponse::Ok().json(TestResult { ok: true, message })),
                Err(e) => Err(AppError::Validation(e)),
            }
        }
        StorageBackend::Minio => {
            let endpoint = body
                .minio_endpoint
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_string();
            let bucket = body
                .minio_bucket
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_string();
            let access = body
                .minio_access_key
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_string();
            let use_ssl = body.minio_use_ssl.unwrap_or(true);

            // Use the submitted secret; if blank, fall back to the stored/env one
            // so a re-test without re-typing the secret still works.
            let secret = match body.minio_secret_key.as_deref() {
                Some(s) if !s.trim().is_empty() => s.trim().to_string(),
                _ => stored_or_env_secret(pool.get_ref()).await,
            };

            match test_minio_storage(&endpoint, &bucket, &access, &secret, use_ssl).await {
                Ok(message) => Ok(HttpResponse::Ok().json(TestResult { ok: true, message })),
                Err(e) => Err(AppError::Validation(e)),
            }
        }
    }
}

#[get("/api/admin/platform/storage/usage")]
pub async fn get_storage_usage(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    ensure_platform_admin(&auth)?;

    let cfg = load_platform_storage_config(pool.get_ref()).await?;
    let workspace_rows = sqlx::query(
        r#"
        SELECT t.id, t.slug, t.name, tb.logo_url
        FROM tenants t
        LEFT JOIN tenant_branding tb ON tb.tenant_id = t.id
        ORDER BY t.created_at ASC
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        AppError::InternalServerError.with_log(format!("list workspaces for storage usage: {e}"))
    })?;

    let mut size_cache: HashMap<String, u64> = HashMap::new();
    let mut items = Vec::with_capacity(workspace_rows.len());

    for row in workspace_rows {
        let workspace_id: Uuid = row.get("id");
        let workspace_slug: String = row.get("slug");
        let workspace_name: String = row.get("name");
        let logo_url: Option<String> = row.get("logo_url");

        let post_rows = sqlx::query("SELECT attachments FROM posts WHERE tenant_id = $1")
            .bind(workspace_id)
            .fetch_all(pool.get_ref())
            .await
            .map_err(|e| {
                AppError::InternalServerError.with_log(format!(
                    "list attachments for workspace {workspace_slug}: {e}"
                ))
            })?;

        let mut logo_urls = HashSet::new();
        let mut attachment_urls = HashSet::new();

        if let Some(url) = logo_url.filter(|value| !value.trim().is_empty()) {
            logo_urls.insert(url);
        }

        for post_row in post_rows {
            let value: serde_json::Value = post_row.get("attachments");
            if let Some(array) = value.as_array() {
                for entry in array {
                    if let Some(url) = entry.as_str() {
                        attachment_urls.insert(url.to_string());
                    }
                }
            }
        }

        let mut logo_bytes = 0_u64;
        for url in &logo_urls {
            logo_bytes += asset_size_bytes(&cfg, url, &mut size_cache)
                .await
                .unwrap_or(0);
        }

        let mut attachment_bytes = 0_u64;
        for url in &attachment_urls {
            attachment_bytes += asset_size_bytes(&cfg, url, &mut size_cache)
                .await
                .unwrap_or(0);
        }

        let asset_count = logo_urls.len() + attachment_urls.len();
        items.push(WorkspaceStorageUsageItem {
            workspace_id,
            workspace_slug,
            workspace_name,
            asset_count,
            total_bytes: logo_bytes + attachment_bytes,
            logo_bytes,
            attachment_bytes,
        });
    }

    let total_assets = items.iter().map(|item| item.asset_count).sum();
    let total_bytes = items.iter().map(|item| item.total_bytes).sum();

    Ok(HttpResponse::Ok().json(WorkspaceStorageUsageResponse {
        backend: cfg.backend,
        total_assets,
        total_bytes,
        items,
    }))
}

#[get("/api/admin/platform/status")]
pub async fn get_platform_status(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    ensure_platform_admin(&auth)?;

    let mut checks = Vec::new();

    checks.push(database_status_check(pool.get_ref()).await);
    checks.push(storage_status_check(pool.get_ref()).await);
    checks.push(ai_status_check(&settings).await);

    let overall = if checks.iter().any(|check| check.level == "fail") {
        "fail"
    } else if checks.iter().any(|check| check.level == "warning") {
        "warning"
    } else {
        "ok"
    };

    Ok(HttpResponse::Ok().json(PlatformStatusResponse {
        overall: overall.to_string(),
        checked_at: Utc::now(),
        checks,
    }))
}

async fn stored_or_env_secret(db: &DbPool) -> String {
    let stored = sqlx::query_scalar::<_, String>(
        "SELECT value FROM system_settings WHERE key = 'storage_minio_secret_key'",
    )
    .fetch_optional(db)
    .await
    .unwrap_or(None)
    .unwrap_or_default();
    if !stored.trim().is_empty() {
        return stored.trim().to_string();
    }
    std::env::var("HOWLLO_MINIO_PASSWORD")
        .unwrap_or_default()
        .trim()
        .to_string()
}

async fn asset_size_bytes(
    cfg: &crate::storage::PlatformStorageConfig,
    public_url: &str,
    cache: &mut HashMap<String, u64>,
) -> Option<u64> {
    if let Some(size) = cache.get(public_url) {
        return Some(*size);
    }

    let relative = public_url
        .strip_prefix(cfg.public_base_url.trim_end_matches('/'))
        .map(|value| value.trim_start_matches('/').to_string())?;

    let size = match cfg.backend {
        StorageBackend::Local => {
            let absolute = format!(
                "{}/{}",
                cfg.local_path.trim_end_matches('/'),
                relative.trim_start_matches('/')
            );
            std::fs::metadata(Path::new(&absolute)).ok()?.len()
        }
        StorageBackend::Minio => {
            let response = reqwest::Client::new().head(public_url).send().await.ok()?;
            if !response.status().is_success() {
                return None;
            }
            response.content_length()?
        }
    };

    cache.insert(public_url.to_string(), size);
    Some(size)
}

async fn database_status_check(pool: &DbPool) -> PlatformStatusCheck {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await
    {
        Ok(_) => {
            let migration_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM _sqlx_migrations WHERE success = TRUE",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            PlatformStatusCheck {
                key: "database".to_string(),
                label: "Database".to_string(),
                level: "ok".to_string(),
                message: "PostgreSQL is reachable.".to_string(),
                detail: Some(format!("{migration_count} migrations applied.")),
            }
        }
        Err(error) => PlatformStatusCheck {
            key: "database".to_string(),
            label: "Database".to_string(),
            level: "fail".to_string(),
            message: "PostgreSQL query failed.".to_string(),
            detail: Some(error.to_string()),
        },
    }
}

async fn storage_status_check(pool: &DbPool) -> PlatformStatusCheck {
    let cfg = match load_platform_storage_config(pool).await {
        Ok(cfg) => cfg,
        Err(error) => {
            return PlatformStatusCheck {
                key: "storage".to_string(),
                label: "File storage".to_string(),
                level: "fail".to_string(),
                message: "Could not load storage configuration.".to_string(),
                detail: Some(error.to_string()),
            };
        }
    };

    let detail = match cfg.backend {
        StorageBackend::Local => format!("Local disk at {}", cfg.local_path.trim()),
        StorageBackend::Minio => format!(
            "MinIO bucket {} via {}",
            cfg.minio_bucket.trim(),
            cfg.minio_endpoint.trim()
        ),
    };

    let result = match cfg.backend {
        StorageBackend::Local => test_local_storage(cfg.local_path.trim()),
        StorageBackend::Minio => {
            let secret = stored_or_env_secret(pool).await;
            test_minio_storage(
                cfg.minio_endpoint.trim(),
                cfg.minio_bucket.trim(),
                cfg.minio_access_key.trim(),
                secret.trim(),
                cfg.minio_use_ssl,
            )
            .await
        }
    };

    match result {
        Ok(message) => PlatformStatusCheck {
            key: "storage".to_string(),
            label: "File storage".to_string(),
            level: "ok".to_string(),
            message,
            detail: Some(detail),
        },
        Err(error) => PlatformStatusCheck {
            key: "storage".to_string(),
            label: "File storage".to_string(),
            level: "fail".to_string(),
            message: "Storage backend check failed.".to_string(),
            detail: Some(format!("{detail}. {error}")),
        },
    }
}

async fn ai_status_check(settings: &Settings) -> PlatformStatusCheck {
    if !settings.ai.enabled {
        return PlatformStatusCheck {
            key: "ai".to_string(),
            label: "AI".to_string(),
            level: "warning".to_string(),
            message: "AI is disabled.".to_string(),
            detail: Some(
                "Enable HOWLLO_AI_ENABLED to test the configured model endpoint.".to_string(),
            ),
        };
    }

    match reqwest::Client::new()
        .get(&settings.ai.base_url)
        .send()
        .await
    {
        Ok(response) => PlatformStatusCheck {
            key: "ai".to_string(),
            label: "AI".to_string(),
            level: "ok".to_string(),
            message: format!("AI endpoint reachable ({})", response.status()),
            detail: Some(format!(
                "Provider: {:?}, model: {}, base URL: {}",
                settings.ai.provider, settings.ai.model, settings.ai.base_url
            )),
        },
        Err(error) => PlatformStatusCheck {
            key: "ai".to_string(),
            label: "AI".to_string(),
            level: "fail".to_string(),
            message: "AI endpoint is unreachable.".to_string(),
            detail: Some(format!(
                "Provider: {:?}, model: {}, base URL: {}. {}",
                settings.ai.provider, settings.ai.model, settings.ai.base_url, error
            )),
        },
    }
}

// ── Database (read-only health check) ──────────────────────────────────────────

#[derive(Serialize)]
pub struct DatabaseInfo {
    pub url_masked: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub connection_ready: bool,
    pub migration_count: i64,
    pub message: String,
}

#[post("/api/admin/platform/database/test")]
pub async fn test_database(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    ensure_platform_admin(&auth)?;

    let parsed = url::Url::parse(&settings.database_url).ok();
    let host = parsed
        .as_ref()
        .and_then(|u| u.host_str())
        .unwrap_or("unknown")
        .to_string();
    let port = parsed.as_ref().and_then(|u| u.port()).unwrap_or(5432);
    let database = parsed
        .as_ref()
        .map(|u| u.path().trim_start_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    let username = parsed
        .as_ref()
        .map(|u| u.username().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());

    // Live read-only checks.
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| AppError::Validation(format!("Cannot query the database: {e}")))?;

    let migration_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(pool.get_ref())
            .await
            .unwrap_or(0);

    Ok(HttpResponse::Ok().json(DatabaseInfo {
        url_masked: format!("postgres://{username}:****@{host}:{port}/{database}"),
        host,
        port,
        database,
        username,
        connection_ready: true,
        migration_count,
        message: format!("Connected. {migration_count} migrations applied."),
    }))
}
