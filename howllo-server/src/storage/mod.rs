//! Platform storage configuration + asset storage.
//!
//! Storage settings follow the env-default → test → save → DB-override pattern:
//! the effective config is read from `system_settings` first, falling back to
//! `HOWLLO_*` env vars. An admin can change the backend (local disk or MinIO),
//! run a real connection test, and on success persist it to the database — from
//! then on the DB value wins over env.
//!
//! The MinIO client is a minimal hand-rolled AWS Signature V4 implementation
//! over reqwest (no S3 SDK). It supports the operations we need: HEAD bucket,
//! PUT/GET/DELETE object, create bucket, and set a public-read policy so a
//! browser can load uploaded screenshots directly.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::path::Path;

use crate::errors::AppError;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    Local,
    Minio,
}

impl StorageBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageBackend::Local => "local",
            StorageBackend::Minio => "minio",
        }
    }

    fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "minio" => StorageBackend::Minio,
            _ => StorageBackend::Local,
        }
    }
}

/// Effective storage config returned to the API. The secret key is never sent —
/// only `minio_secret_key_configured` indicates whether one is stored.
#[derive(Debug, Clone, Serialize)]
pub struct PlatformStorageConfig {
    pub backend: StorageBackend,
    pub backend_configured: bool,
    pub local_path: String,
    pub public_base_url: String,
    pub minio_endpoint: String,
    pub minio_bucket: String,
    pub minio_access_key: String,
    pub minio_secret_key_configured: bool,
    pub minio_use_ssl: bool,
}

/// Incoming save body. Secret key is optional — omit/blank to keep the stored one.
#[derive(Debug, Deserialize)]
pub struct PlatformStorageConfigUpdate {
    pub backend: StorageBackend,
    pub local_path: String,
    pub public_base_url: String,
    pub minio_endpoint: String,
    pub minio_bucket: String,
    pub minio_access_key: String,
    pub minio_secret_key: Option<String>,
    pub minio_use_ssl: bool,
}

// ── DB / env helpers ───────────────────────────────────────────────────────────

async fn get(db: &PgPool, key: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM system_settings WHERE key = $1")
        .bind(key)
        .fetch_optional(db)
        .await
        .unwrap_or(None)
        .unwrap_or_default()
}

async fn set(db: &PgPool, key: &str, value: &str) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO system_settings (key, value, updated_at) VALUES ($1, $2, NOW())
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()",
    )
    .bind(key)
    .bind(value)
    .execute(db)
    .await
    .map_err(|e| AppError::InternalServerError.with_log(format!("save setting '{key}': {e}")))?;
    Ok(())
}

fn env_value(key: &str) -> String {
    std::env::var(key).unwrap_or_default().trim().to_string()
}

/// DB value if non-empty, else env var, else default.
fn pick(db_value: &str, env_key: &str, default: &str) -> String {
    if !db_value.trim().is_empty() {
        return db_value.trim().to_string();
    }
    let env = env_value(env_key);
    if !env.is_empty() {
        return env;
    }
    default.to_string()
}

// ── Load / Save ──────────────────────────────────────────────────────────────

pub async fn load_platform_storage_config(db: &PgPool) -> Result<PlatformStorageConfig, AppError> {
    let backend_setting = get(db, "storage_backend").await;
    let local_path_setting = get(db, "storage_local_path").await;
    let public_base_setting = get(db, "storage_public_base_url").await;
    let endpoint_setting = get(db, "storage_minio_endpoint").await;
    let bucket_setting = get(db, "storage_minio_bucket").await;
    let access_setting = get(db, "storage_minio_access_key").await;
    let secret_setting = get(db, "storage_minio_secret_key").await;
    let ssl_setting = get(db, "storage_minio_use_ssl").await;

    let env_endpoint = env_value("HOWLLO_MINIO_ENDPOINT");
    let env_bucket = env_value("HOWLLO_MINIO_BUCKET");
    let env_access = env_value("HOWLLO_MINIO_USER");

    // Backend: explicit DB setting wins; otherwise infer MinIO when env minio
    // looks configured, else local.
    let backend_configured = !backend_setting.trim().is_empty();
    let backend = if backend_configured {
        StorageBackend::from_str(&backend_setting)
    } else if !env_endpoint.is_empty() && !env_bucket.is_empty() && !env_access.is_empty() {
        StorageBackend::Minio
    } else {
        StorageBackend::Local
    };

    let local_path = pick(&local_path_setting, "HOWLLO_STORAGE_ROOT", "./data/uploads");
    let public_base_url = pick(
        &public_base_setting,
        "HOWLLO_STORAGE_PUBLIC_BASE_URL",
        "http://127.0.0.1:5110/uploads",
    );
    let minio_endpoint = pick(&endpoint_setting, "HOWLLO_MINIO_ENDPOINT", "");
    let minio_bucket = pick(&bucket_setting, "HOWLLO_MINIO_BUCKET", "");
    let minio_access_key = pick(&access_setting, "HOWLLO_MINIO_USER", "");
    let secret_configured =
        !secret_setting.trim().is_empty() || !env_value("HOWLLO_MINIO_PASSWORD").is_empty();
    let minio_use_ssl = if ssl_setting.trim().is_empty() {
        minio_endpoint.trim().starts_with("https://")
    } else {
        ssl_setting.trim() != "false"
    };

    Ok(PlatformStorageConfig {
        backend,
        backend_configured,
        local_path,
        public_base_url,
        minio_endpoint,
        minio_bucket,
        minio_access_key,
        minio_secret_key_configured: secret_configured,
        minio_use_ssl,
    })
}

pub async fn save_platform_storage_config(
    db: &PgPool,
    cfg: &PlatformStorageConfigUpdate,
) -> Result<PlatformStorageConfig, AppError> {
    set(db, "storage_backend", cfg.backend.as_str()).await?;
    set(db, "storage_local_path", cfg.local_path.trim()).await?;
    set(db, "storage_public_base_url", cfg.public_base_url.trim()).await?;
    set(db, "storage_minio_endpoint", cfg.minio_endpoint.trim()).await?;
    set(db, "storage_minio_bucket", cfg.minio_bucket.trim()).await?;
    set(db, "storage_minio_access_key", cfg.minio_access_key.trim()).await?;
    if let Some(secret) = &cfg.minio_secret_key {
        if !secret.trim().is_empty() {
            set(db, "storage_minio_secret_key", secret.trim()).await?;
        }
    }
    set(
        db,
        "storage_minio_use_ssl",
        if cfg.minio_use_ssl { "true" } else { "false" },
    )
    .await?;
    load_platform_storage_config(db).await
}

/// Effective secret: DB value, else env. Never returned to clients.
async fn effective_minio_secret(db: &PgPool) -> String {
    let stored = get(db, "storage_minio_secret_key").await;
    if !stored.trim().is_empty() {
        return stored.trim().to_string();
    }
    env_value("HOWLLO_MINIO_PASSWORD")
}

// ── Public asset storage (used by the upload endpoint) ─────────────────────────

fn public_url(base: &str, relative_path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        relative_path.trim_start_matches('/')
    )
}

/// Store bytes at `relative_path` using the effective backend; returns a public URL.
pub async fn store_public_asset(
    db: &PgPool,
    relative_path: &str,
    bytes: &[u8],
    content_type: &str,
) -> Result<String, AppError> {
    let cfg = load_platform_storage_config(db).await?;
    match cfg.backend {
        StorageBackend::Local => {
            let absolute = format!(
                "{}/{}",
                cfg.local_path.trim_end_matches('/'),
                relative_path.trim_start_matches('/')
            );
            if let Some(parent) = Path::new(&absolute).parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AppError::InternalServerError.with_log(format!("create upload dir: {e}"))
                })?;
            }
            std::fs::write(&absolute, bytes).map_err(|e| {
                AppError::InternalServerError.with_log(format!("write upload: {e}"))
            })?;
        }
        StorageBackend::Minio => {
            let secret = effective_minio_secret(db).await;
            if cfg.minio_endpoint.trim().is_empty()
                || cfg.minio_bucket.trim().is_empty()
                || cfg.minio_access_key.trim().is_empty()
                || secret.trim().is_empty()
            {
                return Err(AppError::InternalServerError
                    .with_log("MinIO selected but configuration is incomplete".into()));
            }
            put_minio_object(
                &cfg.minio_endpoint,
                &cfg.minio_bucket,
                relative_path,
                bytes,
                content_type,
                &cfg.minio_access_key,
                &secret,
                cfg.minio_use_ssl,
            )
            .await
            .map_err(|e| AppError::InternalServerError.with_log(e))?;
        }
    }
    Ok(public_url(&cfg.public_base_url, relative_path))
}

// ── Connection tests ───────────────────────────────────────────────────────────

pub fn test_local_storage(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Local path is required.".into());
    }
    let dir = Path::new(path);
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Cannot create directory '{path}': {e}"))?;
    }
    if !dir.is_dir() {
        return Err(format!("'{path}' exists but is not a directory."));
    }
    let test_file = dir.join(".howllo_storage_test");
    std::fs::write(&test_file, b"howllo-storage-test")
        .map_err(|e| format!("Cannot write to '{path}': {e}"))?;
    std::fs::remove_file(&test_file)
        .map_err(|e| format!("Cannot clean up test file in '{path}': {e}"))?;
    Ok(format!("Local path '{path}' is writable."))
}

/// Full real-world MinIO test: ensure the bucket exists + is public-read, write a
/// probe object (authenticated), read it back ANONYMOUSLY (like a browser would),
/// then delete it. Mirrors rooiam — "connection works" is not enough; uploaded
/// screenshots must be publicly readable.
pub async fn test_minio_storage(
    endpoint: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    use_ssl: bool,
) -> Result<String, String> {
    let endpoint = endpoint.trim();
    let bucket = bucket.trim();
    let access_key = access_key.trim();
    let secret_key = secret_key.trim();
    if endpoint.is_empty() {
        return Err("MinIO endpoint is required.".into());
    }
    if bucket.is_empty() {
        return Err("MinIO bucket name is required.".into());
    }
    if access_key.is_empty() {
        return Err("MinIO access key is required.".into());
    }
    if secret_key.is_empty() {
        return Err("MinIO secret key is required.".into());
    }

    // 1. Ensure bucket exists, then make it public-read for downloads.
    ensure_minio_bucket_exists(endpoint, bucket, access_key, secret_key, use_ssl)
        .await
        .map_err(|e| format!("[BUCKET] {e}"))?;
    set_minio_bucket_public_read(endpoint, bucket, access_key, secret_key, use_ssl)
        .await
        .map_err(|e| format!("[POLICY] {e}"))?;

    let base = build_base(endpoint, use_ssl);
    let probe_key = format!("uploads/_healthcheck/{}.txt", uuid::Uuid::new_v4());
    let probe_body = b"howllo-storage-roundtrip";

    // 2. WRITE (authenticated).
    if let Err(e) = put_minio_object(
        endpoint,
        bucket,
        &probe_key,
        probe_body,
        "text/plain",
        access_key,
        secret_key,
        use_ssl,
    )
    .await
    {
        return Err(format!(
            "[WRITE FAILED] Could not upload a test object to '{bucket}' at {base}: {e}"
        ));
    }

    // 3. READ anonymously (no auth) — exactly what a browser does.
    let read_url = format!("{base}/{bucket}/{probe_key}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("[READ FAILED] build client: {e}"))?;
    let read_result = client.get(&read_url).send().await;

    // 4. DELETE (cleanup, best-effort).
    let delete_err = delete_minio_object(
        endpoint, bucket, &probe_key, access_key, secret_key, use_ssl,
    )
    .await
    .err();

    let read_ok: Result<(), String> = match read_result {
        Ok(resp) => match resp.status().as_u16() {
            200 => {
                let got = resp.bytes().await.unwrap_or_default();
                if got.as_ref() == probe_body {
                    Ok(())
                } else {
                    Err("[READ FAILED] Anonymous read returned 200 but content didn't match — a proxy/CDN is rewriting responses.".into())
                }
            }
            403 => Err(format!(
                "[READ FAILED] Upload worked but anonymous read of {read_url} was DENIED (403). Bucket '{bucket}' is not public-read — browsers can't load uploaded images."
            )),
            404 => Err(format!(
                "[READ FAILED] Upload worked but anonymous read of {read_url} returned 404. Bucket may be private."
            )),
            other => Err(format!(
                "[READ FAILED] Anonymous read of {read_url} returned HTTP {other}."
            )),
        },
        Err(e) => Err(format!(
            "[READ FAILED] Upload worked but could not reach {read_url}: {e}"
        )),
    };

    if let Err(msg) = read_ok {
        return match delete_err {
            Some(de) => Err(format!("{msg} (Also [DELETE FAILED]: {de})")),
            None => Err(msg),
        };
    }

    match delete_err {
        Some(de) => Ok(format!(
            "Round-trip OK: uploaded and read back anonymously (bucket public-read). WARNING [DELETE FAILED]: {de}"
        )),
        None => Ok(format!(
            "Round-trip OK: bucket '{bucket}' is reachable, public-read, and accepts uploads."
        )),
    }
}

// ── MinIO operations (AWS SigV4) ───────────────────────────────────────────────

fn build_base(endpoint: &str, use_ssl: bool) -> String {
    let e = endpoint.trim().trim_end_matches('/');
    if e.starts_with("http://") || e.starts_with("https://") {
        e.to_string()
    } else if use_ssl {
        format!("https://{e}")
    } else {
        format!("http://{e}")
    }
}

fn host_of(url: &str, fallback: &str) -> Result<String, String> {
    Ok(url::Url::parse(url)
        .map_err(|e| format!("Invalid MinIO endpoint URL: {e}"))?
        .host_str()
        .unwrap_or(fallback)
        .to_string())
}

const REGION: &str = "us-east-1";

pub async fn ensure_minio_bucket_exists(
    endpoint: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    use_ssl: bool,
) -> Result<(), String> {
    let base = build_base(endpoint, use_ssl);
    let url = format!("{base}/{}", bucket.trim());
    let host = host_of(&url, endpoint)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("build client: {e}"))?;

    // HEAD: exists?
    {
        let (amz_date, date_stamp) = amz_now();
        let payload_hash = hex_sha256(b"");
        let canonical_headers = format!("host:{host}\nx-amz-date:{amz_date}\n");
        let canonical_request = format!(
            "HEAD\n/{}\n\n{}\nhost;x-amz-date\n{}",
            bucket.trim(),
            canonical_headers,
            payload_hash
        );
        let auth = sign(
            access_key,
            secret_key,
            &date_stamp,
            &amz_date,
            "host;x-amz-date",
            &canonical_request,
        );
        let resp = client
            .head(&url)
            .header("host", &host)
            .header("x-amz-date", &amz_date)
            .header("authorization", auth)
            .send()
            .await
            .map_err(|e| format!("cannot reach MinIO: {e}"))?;
        if resp.status().is_success() {
            return Ok(());
        }
        if resp.status().as_u16() != 404 {
            return Err(format!("HEAD bucket returned {}", resp.status()));
        }
    }

    // PUT: create (us-east-1 needs no body).
    {
        let (amz_date, date_stamp) = amz_now();
        let payload_hash = hex_sha256(b"");
        let canonical_headers =
            format!("content-length:0\ncontent-type:application/octet-stream\nhost:{host}\nx-amz-date:{amz_date}\n");
        let signed = "content-length;content-type;host;x-amz-date";
        let canonical_request = format!(
            "PUT\n/{}\n\n{}\n{}\n{}",
            bucket.trim(),
            canonical_headers,
            signed,
            payload_hash
        );
        let auth = sign(
            access_key,
            secret_key,
            &date_stamp,
            &amz_date,
            signed,
            &canonical_request,
        );
        let resp = client
            .put(&url)
            .header("host", &host)
            .header("x-amz-date", &amz_date)
            .header("content-type", "application/octet-stream")
            .header("content-length", "0")
            .header("authorization", auth)
            .send()
            .await
            .map_err(|e| format!("cannot reach MinIO: {e}"))?;
        let status = resp.status().as_u16();
        if status == 200 || status == 204 || status == 409 {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        Err(format!("PUT bucket returned {status}: {body}"))
    }
}

pub async fn set_minio_bucket_public_read(
    endpoint: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    use_ssl: bool,
) -> Result<(), String> {
    let base = build_base(endpoint, use_ssl);
    let bucket = bucket.trim();
    let policy = format!(
        r#"{{"Version":"2012-10-17","Statement":[{{"Effect":"Allow","Principal":{{"AWS":["*"]}},"Action":["s3:GetObject"],"Resource":["arn:aws:s3:::{bucket}/*"]}}]}}"#
    );
    let body = policy.into_bytes();
    let url = format!("{base}/{bucket}?policy=");
    let host = host_of(&url, endpoint)?;
    let (amz_date, date_stamp) = amz_now();
    let payload_hash = hex_sha256(&body);
    let content_length = body.len().to_string();
    let canonical_headers = format!(
        "content-length:{content_length}\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
    );
    let signed = "content-length;host;x-amz-content-sha256;x-amz-date";
    let canonical_request =
        format!("PUT\n/{bucket}\npolicy=\n{canonical_headers}\n{signed}\n{payload_hash}");
    let auth = sign(
        access_key,
        secret_key,
        &date_stamp,
        &amz_date,
        signed,
        &canonical_request,
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("build client: {e}"))?;
    let resp = client
        .put(&url)
        .header("host", &host)
        .header("x-amz-date", &amz_date)
        .header("x-amz-content-sha256", &payload_hash)
        .header("content-length", content_length)
        .header("authorization", auth)
        .body(body)
        .send()
        .await
        .map_err(|e| format!("cannot reach MinIO: {e}"))?;
    let status = resp.status().as_u16();
    if status == 200 || status == 204 {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("set bucket policy returned {status}: {body}"))
    }
}

#[allow(clippy::too_many_arguments)]
async fn put_minio_object(
    endpoint: &str,
    bucket: &str,
    key: &str,
    body: &[u8],
    content_type: &str,
    access_key: &str,
    secret_key: &str,
    use_ssl: bool,
) -> Result<(), String> {
    let base = build_base(endpoint, use_ssl);
    let bucket = bucket.trim();
    let key = key.trim_start_matches('/');
    let url = format!("{base}/{bucket}/{key}");
    let host = host_of(&url, endpoint)?;
    let (amz_date, date_stamp) = amz_now();
    let payload_hash = hex_sha256(body);
    let content_length = body.len().to_string();
    let canonical_headers = format!(
        "content-length:{content_length}\ncontent-type:{content_type}\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
    );
    let signed = "content-length;content-type;host;x-amz-content-sha256;x-amz-date";
    let canonical_request =
        format!("PUT\n/{bucket}/{key}\n\n{canonical_headers}\n{signed}\n{payload_hash}");
    let auth = sign(
        access_key,
        secret_key,
        &date_stamp,
        &amz_date,
        signed,
        &canonical_request,
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("build client: {e}"))?;
    let resp = client
        .put(&url)
        .header("host", &host)
        .header("x-amz-date", &amz_date)
        .header("x-amz-content-sha256", &payload_hash)
        .header("content-type", content_type)
        .header("content-length", content_length)
        .header("authorization", auth)
        .body(body.to_vec())
        .send()
        .await
        .map_err(|e| format!("cannot reach host {host} ({e})"))?;
    match resp.status().as_u16() {
        200 | 201 | 204 => Ok(()),
        403 => Err(format!(
            "access denied (403) — check keys for bucket '{bucket}'"
        )),
        404 => Err(format!(
            "bucket '{bucket}' not found (404) — create it first"
        )),
        other => {
            let body = resp.text().await.unwrap_or_default();
            Err(format!("PUT returned {other}: {body}"))
        }
    }
}

async fn delete_minio_object(
    endpoint: &str,
    bucket: &str,
    key: &str,
    access_key: &str,
    secret_key: &str,
    use_ssl: bool,
) -> Result<(), String> {
    let base = build_base(endpoint, use_ssl);
    let bucket = bucket.trim();
    let key = key.trim_start_matches('/');
    let url = format!("{base}/{bucket}/{key}");
    let host = host_of(&url, endpoint)?;
    let (amz_date, date_stamp) = amz_now();
    let payload_hash = hex_sha256(b"");
    let canonical_headers =
        format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
    let signed = "host;x-amz-content-sha256;x-amz-date";
    let canonical_request =
        format!("DELETE\n/{bucket}/{key}\n\n{canonical_headers}\n{signed}\n{payload_hash}");
    let auth = sign(
        access_key,
        secret_key,
        &date_stamp,
        &amz_date,
        signed,
        &canonical_request,
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("build client: {e}"))?;
    let resp = client
        .delete(&url)
        .header("host", &host)
        .header("x-amz-date", &amz_date)
        .header("x-amz-content-sha256", &payload_hash)
        .header("authorization", auth)
        .send()
        .await
        .map_err(|e| format!("cannot reach MinIO: {e}"))?;
    match resp.status().as_u16() {
        200 | 204 | 404 => Ok(()),
        other => {
            let body = resp.text().await.unwrap_or_default();
            Err(format!("DELETE returned {other}: {body}"))
        }
    }
}

// ── SigV4 helpers ──────────────────────────────────────────────────────────────

fn amz_now() -> (String, String) {
    let now = chrono::Utc::now();
    (
        now.format("%Y%m%dT%H%M%SZ").to_string(),
        now.format("%Y%m%d").to_string(),
    )
}

/// Build the full `Authorization` header for an S3 request given the canonical request.
fn sign(
    access_key: &str,
    secret_key: &str,
    date_stamp: &str,
    amz_date: &str,
    signed_headers: &str,
    canonical_request: &str,
) -> String {
    let credential_scope = format!("{date_stamp}/{REGION}/s3/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        hex_sha256(canonical_request.as_bytes())
    );
    let signing_key = derive_signing_key(secret_key, date_stamp, REGION, "s3");
    let signature = hex_hmac_sha256(&signing_key, string_to_sign.as_bytes());
    format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{credential_scope},SignedHeaders={signed_headers},Signature={signature}"
    )
}

fn hex_sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    const BLOCK: usize = 64;
    let key_block = if key.len() > BLOCK {
        let mut h = Sha256::new();
        h.update(key);
        let hash = h.finalize();
        let mut b = [0u8; BLOCK];
        b[..hash.len()].copy_from_slice(&hash);
        b.to_vec()
    } else {
        let mut b = vec![0u8; BLOCK];
        b[..key.len()].copy_from_slice(key);
        b
    };
    let ipad: Vec<u8> = key_block.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = key_block.iter().map(|b| b ^ 0x5c).collect();
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(data);
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(inner_hash.as_slice());
    outer.finalize().to_vec()
}

fn hex_hmac_sha256(key: &[u8], data: &[u8]) -> String {
    hex::encode(hmac_sha256(key, data))
}

fn derive_signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}
