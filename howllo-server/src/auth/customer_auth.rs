//! Per-workspace sign-in options for public board members.
use actix_web::{delete, get, patch, put, web, HttpResponse, Responder};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    current_user::AuthenticatedUser,
    providers::{OidcProviderConfig, PublicProvider},
};
use crate::{db::DbPool, errors::AppError};

#[derive(Deserialize)]
pub struct SlugQuery {
    pub tenant_slug: String,
}

#[derive(Serialize)]
pub struct CustomerAuthSettings {
    local_enabled: bool,
    rooiam_enabled: bool,
    rooiam_workspace_id: Option<String>,
    rooiam_client_id: Option<String>,
    rooiam_widget_base_url: Option<String>,
    providers: Vec<CustomerOidcSetting>,
    callback_urls: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
struct CustomerOidcSetting {
    key: String,
    display_name: String,
    issuer: String,
    client_id: String,
    has_client_secret: bool,
    token_endpoint_auth_method: String,
    enabled: bool,
    callback_url: String,
}

#[derive(Deserialize)]
pub struct UpdateCustomerAuth {
    local_enabled: bool,
    rooiam_enabled: bool,
    rooiam_workspace_id: Option<String>,
    rooiam_client_id: Option<String>,
}

#[derive(Deserialize)]
pub struct SaveOidcProvider {
    display_name: String,
    issuer: String,
    client_id: String,
    client_secret: Option<String>,
    token_endpoint_auth_method: Option<String>,
    enabled: bool,
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(%error, "customer sign-in storage error");
    AppError::InternalServerError
}

fn key() -> Result<Aes256Gcm, AppError> {
    let raw = std::env::var("HOWLLO_OIDC_CONFIG_KEY").map_err(|_| AppError::InternalServerError)?;
    let bytes = hex::decode(raw).map_err(|_| AppError::InternalServerError)?;
    if bytes.len() != 32 {
        return Err(AppError::InternalServerError);
    }
    Aes256Gcm::new_from_slice(&bytes).map_err(|_| AppError::InternalServerError)
}

fn encrypt(secret: &str) -> Result<String, AppError> {
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = key()?
        .encrypt(Nonce::from_slice(&nonce), secret.as_bytes())
        .map_err(|_| AppError::InternalServerError)?;
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(nonce),
        URL_SAFE_NO_PAD.encode(ciphertext)
    ))
}

fn decrypt(ciphertext: &str) -> Result<String, AppError> {
    let (nonce, body) = ciphertext
        .split_once('.')
        .ok_or(AppError::InternalServerError)?;
    let nonce = URL_SAFE_NO_PAD
        .decode(nonce)
        .map_err(|_| AppError::InternalServerError)?;
    let body = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| AppError::InternalServerError)?;
    if nonce.len() != 12 {
        return Err(AppError::InternalServerError);
    }
    let plain = key()?
        .decrypt(Nonce::from_slice(&nonce), body.as_ref())
        .map_err(|_| AppError::InternalServerError)?;
    String::from_utf8(plain).map_err(|_| AppError::InternalServerError)
}

fn provider_key_valid(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn issuer_allowed(provider_key: &str, value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    if url.scheme() != "https"
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    match provider_key {
        "google" => value.trim_end_matches('/') == "https://accounts.google.com",
        "microsoft" => {
            url.host_str() == Some("login.microsoftonline.com")
                && url.path_segments().is_some_and(|mut parts| {
                    let directory = parts.next().unwrap_or_default();
                    let version = parts.next().unwrap_or_default();
                    Uuid::parse_str(directory).is_ok()
                        && version == "v2.0"
                        && parts.next().is_none()
                })
        }
        _ => std::env::var("HOWLLO_OIDC_ALLOWED_ISSUERS")
            .unwrap_or_default()
            .split(',')
            .any(|allowed| {
                allowed.trim().trim_end_matches('/') == value.trim_end_matches('/')
                    && !allowed.trim().is_empty()
            }),
    }
}

fn callback_url(id: &str) -> Result<String, AppError> {
    let origin =
        std::env::var("HOWLLO_PUBLIC_API_URL").map_err(|_| AppError::InternalServerError)?;
    Ok(format!(
        "{}/api/auth/callback/{id}",
        origin.trim_end_matches('/')
    ))
}

pub fn scoped_provider_id(tenant_id: Uuid, key: &str) -> String {
    format!("ws-{}-{key}", tenant_id.simple())
}

pub fn parse_scoped_provider_id(id: &str) -> Option<(Uuid, String)> {
    let rest = id.strip_prefix("ws-")?;
    let (tenant, key) = rest.split_once('-')?;
    if tenant.len() != 32 || !provider_key_valid(key) {
        return None;
    }
    Some((Uuid::parse_str(tenant).ok()?, key.to_string()))
}

pub async fn scoped_oidc_provider(pool: &DbPool, id: &str) -> Result<OidcProviderConfig, AppError> {
    let (tenant_id, provider_key) = parse_scoped_provider_id(id).ok_or(AppError::NotFound)?;
    let row: Option<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT display_name, issuer, client_id, client_secret_ciphertext, token_endpoint_auth_method FROM workspace_oidc_providers WHERE tenant_id = $1 AND provider_key = $2 AND enabled = TRUE"
    ).bind(tenant_id).bind(&provider_key).fetch_optional(pool).await.map_err(db_error)?;
    let (display_name, issuer, client_id, ciphertext, auth_method) =
        row.ok_or(AppError::NotFound)?;
    Ok(OidcProviderConfig {
        id: id.to_string(),
        display_name,
        issuer,
        client_id,
        client_secret: Some(decrypt(&ciphertext)?),
        token_endpoint_auth_method: Some(auth_method),
        scopes: "openid profile email".into(),
        enabled: true,
    })
}

pub async fn is_customer_provider_enabled(
    pool: &DbPool,
    tenant_id: Uuid,
    provider: &str,
) -> Result<bool, AppError> {
    if provider == "local" || provider == "rooiam" {
        let row: Option<(bool, bool)> = sqlx::query_as(
            "SELECT local_enabled, rooiam_enabled FROM workspace_customer_auth WHERE tenant_id = $1"
        ).bind(tenant_id).fetch_optional(pool).await.map_err(db_error)?;
        return Ok(if provider == "local" {
            row.map_or(true, |r| r.0)
        } else {
            row.is_some_and(|r| r.1)
        });
    }
    let Some((scope, key)) = parse_scoped_provider_id(provider) else {
        return Ok(super::providers::configured_oidc()?
            .iter()
            .any(|item| item.id == provider && item.enabled));
    };
    if scope != tenant_id {
        return Ok(false);
    }
    let enabled: Option<bool> = sqlx::query_scalar(
        "SELECT enabled FROM workspace_oidc_providers WHERE tenant_id = $1 AND provider_key = $2",
    )
    .bind(tenant_id)
    .bind(key)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    Ok(enabled.unwrap_or(false))
}

pub async fn customer_rooiam_config(
    pool: &DbPool,
    tenant_slug: &str,
) -> Result<Option<(String, String)>, AppError> {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT a.rooiam_workspace_id, a.rooiam_client_id FROM workspace_customer_auth a JOIN tenants t ON t.id = a.tenant_id WHERE t.slug = $1 AND a.rooiam_enabled = TRUE"
    ).bind(tenant_slug).fetch_optional(pool).await.map_err(db_error)?;
    Ok(row.and_then(|(workspace, client)| Some((workspace?, client?))))
}

pub async fn verify_customer_rooiam_token(
    pool: &DbPool,
    tenant_id: Uuid,
    token: &str,
) -> Result<bool, AppError> {
    let client_id: Option<String> = sqlx::query_scalar(
        "SELECT rooiam_client_id FROM workspace_customer_auth WHERE tenant_id=$1 AND rooiam_enabled=TRUE"
    ).bind(tenant_id).fetch_optional(pool).await.map_err(db_error)?.flatten();
    let Some(client_id) = client_id else {
        return Ok(false);
    };
    let issuer =
        std::env::var("HOWLLO_ROOIAM_ISSUER").unwrap_or_else(|_| "https://api.rooiam.com".into());
    let endpoint = format!("{}/v1/oidc/introspect", issuer.trim_end_matches('/'));
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AppError::InternalServerError)?
        .post(endpoint)
        .form(&[("token", token), ("client_id", client_id.as_str())])
        .send()
        .await
        .map_err(|_| AppError::Unauthorized)?;
    if !response.status().is_success() {
        return Ok(false);
    }
    #[derive(Deserialize)]
    struct Introspection {
        active: bool,
        client_id: Option<String>,
    }
    let info = response
        .json::<Introspection>()
        .await
        .map_err(|_| AppError::Unauthorized)?;
    Ok(info.active && info.client_id.as_deref() == Some(client_id.as_str()))
}

#[get("/api/auth/customer-rooiam")]
pub async fn public_rooiam(
    pool: web::Data<DbPool>,
    query: web::Query<SlugQuery>,
) -> Result<impl Responder, AppError> {
    let config = customer_rooiam_config(pool.get_ref(), &query.tenant_slug).await?;
    let Some((workspace_id, client_id)) = config else {
        return Err(AppError::NotFound);
    };
    let widget_base_url = std::env::var("HOWLLO_ROOIAM_WIDGET_BASE_URL")
        .unwrap_or_else(|_| "https://api.rooiam.com/login-widget".into());
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "provider": "rooiam", "rooiam_workspace_id": workspace_id,
        "rooiam_client_id": client_id, "rooiam_widget_base_url": widget_base_url,
    })))
}

#[get("/api/auth/customer-providers")]
pub async fn public_providers(
    pool: web::Data<DbPool>,
    query: web::Query<SlugQuery>,
) -> Result<impl Responder, AppError> {
    let tenant_id = crate::repositories::membership_repository::resolve_tenant_id(
        pool.get_ref(),
        &query.tenant_slug,
    )
    .await?;
    let mut providers = Vec::new();
    if super::providers::local_enabled()
        && is_customer_provider_enabled(pool.get_ref(), tenant_id, "local").await?
    {
        providers.push(PublicProvider {
            id: "local".into(),
            display_name: "Password".into(),
            kind: "local",
            login_url: "/api/auth/local/login".into(),
        });
    }
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT provider_key, display_name FROM workspace_oidc_providers WHERE tenant_id = $1 AND enabled = TRUE ORDER BY provider_key"
    ).bind(tenant_id).fetch_all(pool.get_ref()).await.map_err(db_error)?;
    for (key, display_name) in rows {
        let id = scoped_provider_id(tenant_id, &key);
        providers.push(PublicProvider {
            login_url: format!("/api/auth/login/{id}"),
            id,
            display_name,
            kind: "oidc",
        });
    }
    Ok(HttpResponse::Ok().json(providers))
}

async fn settings(pool: &DbPool, tenant_id: Uuid) -> Result<CustomerAuthSettings, AppError> {
    let row: Option<(bool, bool, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT local_enabled, rooiam_enabled, rooiam_workspace_id, rooiam_client_id FROM workspace_customer_auth WHERE tenant_id = $1"
    ).bind(tenant_id).fetch_optional(pool).await.map_err(db_error)?;
    let (local_enabled, rooiam_enabled, rooiam_workspace_id, rooiam_client_id) =
        row.unwrap_or((true, false, None, None));
    let rows: Vec<(String, String, String, String, String, bool)> = sqlx::query_as(
        "SELECT provider_key, display_name, issuer, client_id, token_endpoint_auth_method, enabled FROM workspace_oidc_providers WHERE tenant_id = $1 ORDER BY provider_key"
    ).bind(tenant_id).fetch_all(pool).await.map_err(db_error)?;
    let mut providers = Vec::new();
    for (key, display_name, issuer, client_id, token_endpoint_auth_method, enabled) in rows {
        providers.push(CustomerOidcSetting {
            callback_url: callback_url(&scoped_provider_id(tenant_id, &key))?,
            key,
            display_name,
            issuer,
            client_id,
            token_endpoint_auth_method,
            has_client_secret: true,
            enabled,
        });
    }
    Ok(CustomerAuthSettings {
        local_enabled,
        rooiam_enabled,
        rooiam_workspace_id,
        rooiam_client_id,
        rooiam_widget_base_url: if rooiam_enabled {
            std::env::var("HOWLLO_ROOIAM_WIDGET_BASE_URL")
                .ok()
                .or(Some("https://api.rooiam.com/login-widget".into()))
        } else {
            None
        },
        providers,
        callback_urls: ["google", "microsoft", "oidc"]
            .into_iter()
            .map(|key| {
                Ok((
                    key.to_string(),
                    callback_url(&scoped_provider_id(tenant_id, key))?,
                ))
            })
            .collect::<Result<_, AppError>>()?,
    })
}

async fn manager_tenant(
    pool: &DbPool,
    slug: &str,
    auth: &AuthenticatedUser,
) -> Result<Uuid, AppError> {
    let tenant_id =
        crate::repositories::membership_repository::resolve_tenant_id(pool, slug).await?;
    super::require_admin(pool, tenant_id, auth.0.id).await?;
    Ok(tenant_id)
}

#[get("/api/tenants/{slug}/customer-auth")]
pub async fn get_settings(
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = manager_tenant(pool.get_ref(), &path, &auth).await?;
    Ok(HttpResponse::Ok().json(settings(pool.get_ref(), tenant_id).await?))
}

#[patch("/api/tenants/{slug}/customer-auth")]
pub async fn update_settings(
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    auth: AuthenticatedUser,
    body: web::Json<UpdateCustomerAuth>,
) -> Result<impl Responder, AppError> {
    let tenant_id = manager_tenant(pool.get_ref(), &path, &auth).await?;
    let workspace = body
        .rooiam_workspace_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let client = body
        .rooiam_client_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    if body.rooiam_enabled && (workspace.is_none() || client.is_none()) {
        return Err(AppError::Validation(
            "RooIAM workspace ID and client ID are required".into(),
        ));
    }
    if workspace.is_some_and(|value| Uuid::parse_str(value).is_err())
        || client.is_some_and(|value| value.len() > 256 || !value.is_ascii())
    {
        return Err(AppError::Validation(
            "Invalid RooIAM workspace ID or client ID".into(),
        ));
    }
    if !body.local_enabled && !body.rooiam_enabled {
        let other: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_oidc_providers WHERE tenant_id = $1 AND enabled = TRUE)")
            .bind(tenant_id).fetch_one(pool.get_ref()).await.map_err(db_error)?;
        if !other {
            return Err(AppError::Validation(
                "Enable at least one customer sign-in method".into(),
            ));
        }
    }
    sqlx::query("INSERT INTO workspace_customer_auth (tenant_id, local_enabled, rooiam_enabled, rooiam_workspace_id, rooiam_client_id) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (tenant_id) DO UPDATE SET local_enabled=$2, rooiam_enabled=$3, rooiam_workspace_id=$4, rooiam_client_id=$5, updated_at=NOW()")
        .bind(tenant_id).bind(body.local_enabled).bind(body.rooiam_enabled).bind(workspace).bind(client)
        .execute(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::Ok().json(settings(pool.get_ref(), tenant_id).await?))
}

#[put("/api/tenants/{slug}/customer-auth/providers/{key}")]
pub async fn save_provider(
    pool: web::Data<DbPool>,
    path: web::Path<(String, String)>,
    auth: AuthenticatedUser,
    body: web::Json<SaveOidcProvider>,
) -> Result<impl Responder, AppError> {
    let (slug, provider_key) = path.into_inner();
    let tenant_id = manager_tenant(pool.get_ref(), &slug, &auth).await?;
    if !provider_key_valid(&provider_key)
        || matches!(provider_key.as_str(), "local" | "rooiam")
        || body.display_name.trim().is_empty()
        || body.display_name.chars().count() > 60
        || body.client_id.trim().is_empty()
        || body.client_id.len() > 512
        || !issuer_allowed(&provider_key, body.issuer.trim())
        || !matches!(
            body.token_endpoint_auth_method
                .as_deref()
                .unwrap_or("client_secret_post"),
            "client_secret_post" | "client_secret_basic"
        )
    {
        return Err(AppError::Validation(
            "Invalid provider settings or issuer is not allowed by the server".into(),
        ));
    }
    let ciphertext = if let Some(secret) = body
        .client_secret
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        if secret.len() > 2048 {
            return Err(AppError::Validation("Client secret is too long".into()));
        }
        encrypt(secret)?
    } else {
        sqlx::query_scalar::<_, String>("SELECT client_secret_ciphertext FROM workspace_oidc_providers WHERE tenant_id=$1 AND provider_key=$2")
            .bind(tenant_id).bind(&provider_key).fetch_optional(pool.get_ref()).await.map_err(db_error)?
            .ok_or(AppError::Validation("Client secret is required".into()))?
    };
    sqlx::query("INSERT INTO workspace_oidc_providers (tenant_id, provider_key, display_name, issuer, client_id, client_secret_ciphertext, token_endpoint_auth_method, enabled) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (tenant_id,provider_key) DO UPDATE SET display_name=$3, issuer=$4, client_id=$5, client_secret_ciphertext=$6, token_endpoint_auth_method=$7, enabled=$8, updated_at=NOW()")
        .bind(tenant_id).bind(&provider_key).bind(body.display_name.trim()).bind(body.issuer.trim().trim_end_matches('/'))
        .bind(body.client_id.trim()).bind(ciphertext).bind(body.token_endpoint_auth_method.as_deref().unwrap_or("client_secret_post")).bind(body.enabled).execute(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::Ok().json(settings(pool.get_ref(), tenant_id).await?))
}

#[delete("/api/tenants/{slug}/customer-auth/providers/{key}")]
pub async fn delete_provider(
    pool: web::Data<DbPool>,
    path: web::Path<(String, String)>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let (slug, provider_key) = path.into_inner();
    let tenant_id = manager_tenant(pool.get_ref(), &slug, &auth).await?;
    sqlx::query("DELETE FROM workspace_oidc_providers WHERE tenant_id=$1 AND provider_key=$2")
        .bind(tenant_id)
        .bind(provider_key)
        .execute(pool.get_ref())
        .await
        .map_err(db_error)?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db,
        http::test_support::{
            bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
        },
    };
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;

    #[actix_web::test]
    async fn scoped_provider_names_cannot_cross_workspaces() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let id = scoped_provider_id(first, "google");
        assert_eq!(
            parse_scoped_provider_id(&id),
            Some((first, "google".into()))
        );
        assert_ne!(parse_scoped_provider_id(&id).unwrap().0, second);
        assert!(parse_scoped_provider_id("ws-invalid-google").is_none());
        assert!(!issuer_allowed(
            "google",
            "https://accounts.google.com.evil.example"
        ));
        assert!(!issuer_allowed(
            "microsoft",
            "https://login.microsoftonline.com/common/v2.0"
        ));
    }

    #[actix_web::test]
    async fn tenant_provider_secret_is_encrypted_and_scoped() {
        let _guard = lock_test_db().await;
        let test_config = test_settings();
        let pool = db::establish_connection(&test_config.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;
        let tenant_id: Uuid = sqlx::query_scalar("SELECT id FROM tenants WHERE slug=$1")
            .bind(&seed.tenant_slug)
            .fetch_one(&pool)
            .await
            .unwrap();
        let other = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Other')")
            .bind(other)
            .bind(format!("other-{}", other.simple()))
            .execute(&pool)
            .await
            .unwrap();
        std::env::set_var("HOWLLO_OIDC_CONFIG_KEY", "0f".repeat(32));
        std::env::set_var("HOWLLO_PUBLIC_API_URL", "http://localhost:7700");
        let secret = "test-tenant-secret";
        let bearer = bearer_for(
            &seed.admin_subject,
            &format!("{}@example.com", seed.admin_subject),
            "Admin",
            &test_config.rooiam_jwt_secret,
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(test_config))
                .configure(crate::startup::configure),
        )
        .await;
        let response = test::call_service(&app, test::TestRequest::put()
            .uri(&format!("/api/tenants/{}/customer-auth/providers/google", seed.tenant_slug))
            .insert_header(("Authorization", bearer))
            .set_json(json!({"display_name":"Google","issuer":"https://accounts.google.com","client_id":"test-client","client_secret":secret,"enabled":true}))
            .to_request()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!read_json(response).await.to_string().contains(secret));
        let ciphertext: String = sqlx::query_scalar("SELECT client_secret_ciphertext FROM workspace_oidc_providers WHERE tenant_id=$1 AND provider_key='google'")
            .bind(tenant_id).fetch_one(&pool).await.unwrap();
        assert!(!ciphertext.contains(secret));
        let public_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/auth/customer-providers?tenant_slug={}",
                    seed.tenant_slug
                ))
                .to_request(),
        )
        .await;
        assert_eq!(public_response.status(), StatusCode::OK);
        let public_list = read_json(public_response).await;
        assert!(public_list.to_string().contains("Google"));
        assert!(!public_list.to_string().contains(secret));
        let id = scoped_provider_id(tenant_id, "google");
        let loaded = scoped_oidc_provider(&pool, &id).await.unwrap();
        assert_eq!(loaded.client_secret.as_deref(), Some(secret));
        assert!(is_customer_provider_enabled(&pool, tenant_id, &id)
            .await
            .unwrap());
        assert!(!is_customer_provider_enabled(&pool, other, &id)
            .await
            .unwrap());
        let public = settings(&pool, tenant_id).await.unwrap();
        assert!(!serde_json::to_string(&public).unwrap().contains(secret));
        std::env::remove_var("HOWLLO_OIDC_CONFIG_KEY");
        std::env::remove_var("HOWLLO_PUBLIC_API_URL");
    }

    #[actix_web::test]
    async fn scoped_oidc_account_cannot_join_another_workspace() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let own = seed_basic_tenant(&pool).await;
        let other = seed_basic_tenant(&pool).await;
        let tenant_id: Uuid = sqlx::query_scalar("SELECT id FROM tenants WHERE slug=$1")
            .bind(&own.tenant_slug)
            .fetch_one(&pool)
            .await
            .unwrap();
        let id = scoped_provider_id(tenant_id, "google");
        sqlx::query("INSERT INTO workspace_oidc_providers (tenant_id, provider_key, display_name, issuer, client_id, client_secret_ciphertext, enabled) VALUES ($1,'google','Google','https://accounts.google.com','test-client','ciphertext',TRUE)")
            .bind(tenant_id).execute(&pool).await.unwrap();
        let token = crate::auth::local_user::issue_scoped_account_token(
            &pool,
            own.member_user_id,
            Some(tenant_id),
            &id,
        )
        .await
        .unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .app_data(web::Data::new(settings))
                .configure(crate::startup::configure),
        )
        .await;
        let own_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/workspace-session")
                .insert_header(("Authorization", format!("Bearer {}", token.access_token)))
                .set_json(json!({"tenant_slug": own.tenant_slug}))
                .to_request(),
        )
        .await;
        assert_eq!(own_response.status(), StatusCode::CREATED);
        let other_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/workspace-session")
                .insert_header(("Authorization", format!("Bearer {}", token.access_token)))
                .set_json(json!({"tenant_slug": other.tenant_slug}))
                .to_request(),
        )
        .await;
        assert_eq!(other_response.status(), StatusCode::FORBIDDEN);
    }
}
