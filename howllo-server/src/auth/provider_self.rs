//! Optional, provider-neutral self-service bridge for a signed-in workspace member.
//! Provider credentials stay encrypted on the API server and never appear in API responses.
use actix_web::{web, HttpRequest, HttpResponse};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use rand::{rngs::OsRng, RngCore};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{config::Settings, db::DbPool, errors::AppError};

fn cipher() -> Result<Aes256Gcm, AppError> {
    let key = std::env::var("HOWLLO_IDENTITY_TOKEN_KEY")
        .or_else(|_| std::env::var("HOWLLO_OIDC_CONFIG_KEY"))
        .map_err(|_| {
            AppError::ServiceUnavailable("Identity token encryption key is not configured.".into())
        })?;
    let bytes = hex::decode(key).map_err(|_| AppError::InternalServerError)?;
    if bytes.len() != 32 {
        return Err(AppError::InternalServerError);
    }
    Aes256Gcm::new_from_slice(&bytes).map_err(|_| AppError::InternalServerError)
}

pub(crate) fn encrypt_token(token: &str) -> Result<String, AppError> {
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher()?
        .encrypt(Nonce::from_slice(&nonce), token.as_bytes())
        .map_err(|_| AppError::InternalServerError)?;
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(nonce),
        URL_SAFE_NO_PAD.encode(encrypted)
    ))
}

fn decrypt_token(token: &str) -> Result<String, AppError> {
    let (nonce, data) = token.split_once('.').ok_or(AppError::InternalServerError)?;
    let nonce = URL_SAFE_NO_PAD
        .decode(nonce)
        .map_err(|_| AppError::InternalServerError)?;
    let data = URL_SAFE_NO_PAD
        .decode(data)
        .map_err(|_| AppError::InternalServerError)?;
    if nonce.len() != 12 {
        return Err(AppError::InternalServerError);
    }
    let plain = cipher()?
        .decrypt(Nonce::from_slice(&nonce), data.as_ref())
        .map_err(|_| AppError::InternalServerError)?;
    String::from_utf8(plain).map_err(|_| AppError::InternalServerError)
}

/// Only call after validating the access token with the configured provider.
pub(crate) fn token_session_id(token: &str) -> Option<Uuid> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    value.get("sid")?.as_str()?.parse().ok()
}

fn token_hash(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

fn raw_session(req: &HttpRequest) -> Result<&str, AppError> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;
    if !super::workspace_session::is_workspace_session_token(token) {
        return Err(AppError::Unauthorized);
    }
    Ok(token)
}

async fn provider_token(
    req: &HttpRequest,
    pool: &DbPool,
    settings: &Settings,
) -> Result<(String, Uuid, Uuid, String), AppError> {
    let raw = raw_session(req)?;
    let auth = super::workspace_session::resolve_workspace_session_from_token(pool, settings, raw)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !crate::external_identity::linked_workspace(pool, settings, auth.tenant_id).await? {
        return Err(AppError::NotFound);
    }
    let hash = token_hash(raw);
    let access =
        provider_token_for_session(pool, settings, auth.user.id, auth.tenant_id, &hash).await?;
    Ok((access, auth.user.id, auth.tenant_id, hash))
}

async fn provider_token_for_session(
    pool: &DbPool,
    settings: &Settings,
    user_id: Uuid,
    tenant_id: Uuid,
    hash: &str,
) -> Result<String, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError)?;
    let row: Option<(Option<String>, Option<String>, Option<DateTime<Utc>>, Option<Uuid>, bool)> = sqlx::query_as(
        "SELECT provider_access_ciphertext, provider_refresh_ciphertext, provider_access_expires_at, provider_session_id, provider_token_pending_validation FROM workspace_sessions WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>NOW() FOR UPDATE"
    ).bind(&hash).fetch_optional(&mut *tx).await.map_err(|_| AppError::InternalServerError)?;
    let Some((Some(access), refresh, expires, provider_sid, pending_validation)) = row else {
        return Err(AppError::NotFound);
    };
    let mut access = decrypt_token(&access)?;
    let mut rotated = false;
    if expires.is_none_or(|time| time <= Utc::now() + Duration::seconds(30)) {
        let refresh = refresh.ok_or(AppError::Unauthorized)?;
        let client_id: String = sqlx::query_scalar(
            "SELECT rooiam_client_id FROM workspace_customer_auth WHERE tenant_id=$1 AND rooiam_enabled=TRUE"
        ).bind(tenant_id).fetch_optional(&mut *tx).await.map_err(|_| AppError::InternalServerError)?
            .flatten().ok_or(AppError::Forbidden)?;
        let refreshed = crate::external_identity::call_bridge(settings, reqwest::Method::POST,
            "/v1/self/token/refresh", None,
            Some(&serde_json::json!({ "refresh_token": decrypt_token(&refresh)?, "client_id": client_id })),
        ).await?;
        let new_access = refreshed
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or(AppError::Unauthorized)?;
        let new_refresh = refreshed
            .get("refresh_token")
            .and_then(Value::as_str)
            .ok_or(AppError::Unauthorized)?;
        if provider_sid.is_none() || provider_sid != token_session_id(new_access) {
            sqlx::query("UPDATE workspace_sessions SET revoked_at=NOW() WHERE token_hash=$1")
                .bind(&hash)
                .execute(&mut *tx)
                .await
                .map_err(|_| AppError::InternalServerError)?;
            tx.commit()
                .await
                .map_err(|_| AppError::InternalServerError)?;
            return Err(AppError::Unauthorized);
        }
        let until = Utc::now()
            + Duration::seconds(
                refreshed
                    .get("expires_in")
                    .and_then(Value::as_i64)
                    .unwrap_or(3600)
                    .clamp(60, 3600),
            );
        sqlx::query("UPDATE workspace_sessions SET provider_access_ciphertext=$2, provider_refresh_ciphertext=$3, provider_access_expires_at=$4, provider_token_pending_validation=TRUE WHERE token_hash=$1")
            .bind(&hash).bind(encrypt_token(new_access)?).bind(encrypt_token(new_refresh)?)
            .bind(until).execute(&mut *tx).await.map_err(|_| AppError::InternalServerError)?;
        access = new_access.to_string();
        rotated = true;
    }
    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError)?;
    if rotated || pending_validation {
        // Commit rotated credentials before making any more network calls.
        // A transient validation outage must not replay an already-used refresh
        // token and trigger RooIAM's refresh-token-family reuse protection.
        let valid =
            super::customer_auth::verify_customer_rooiam_token(pool, tenant_id, &access).await?;
        let identity = super::rooiam::resolve_rooiam_access_token(settings, &access).await?;
        let subject: Option<String> = sqlx::query_scalar(
            "SELECT subject FROM user_identities WHERE user_id=$1 AND provider_id=$2",
        )
        .bind(user_id)
        .bind(crate::external_identity::provider_id(settings)?)
        .fetch_optional(pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;
        if !valid || subject.as_deref() != Some(identity.sub.as_str()) {
            sqlx::query("UPDATE workspace_sessions SET revoked_at=NOW() WHERE token_hash=$1")
                .bind(hash)
                .execute(pool)
                .await
                .map_err(|_| AppError::InternalServerError)?;
            return Err(AppError::Unauthorized);
        }
        let updated = sqlx::query("UPDATE workspace_sessions SET provider_token_pending_validation=FALSE WHERE token_hash=$1 AND revoked_at IS NULL")
            .bind(hash).execute(pool).await.map_err(|_| AppError::InternalServerError)?;
        if updated.rows_affected() == 0 {
            return Err(AppError::Unauthorized);
        }
    }
    Ok(access)
}

/// Verify that a linked provider session remains live before accepting its
/// associated Howllo session. Called by the existing two-minute session check.
pub(crate) async fn provider_session_active(
    pool: &DbPool,
    settings: &Settings,
    user_id: Uuid,
    tenant_id: Uuid,
    hash: &str,
) -> Result<bool, AppError> {
    let has_provider_token: bool = sqlx::query_scalar(
        "SELECT provider_access_ciphertext IS NOT NULL FROM workspace_sessions WHERE token_hash=$1",
    )
    .bind(hash)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::InternalServerError)?
    .unwrap_or(false);
    if !has_provider_token {
        // Staff sign-in has its own account flow. Only customer sessions need
        // this user-scoped token vault for self-service and revocation checks.
        let staff: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM memberships WHERE tenant_id=$1 AND user_id=$2 AND role IN ('owner','admin','moderator'))")
            .bind(tenant_id).bind(user_id).fetch_one(pool).await.map_err(|_| AppError::InternalServerError)?;
        if staff {
            return Ok(true);
        }
        // Older customer sessions cannot prove that their upstream session is
        // still live. Require a fresh sign-in; local sessions keep working.
        let provider_id = crate::external_identity::provider_id(settings)?;
        let external: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_identities WHERE user_id=$1 AND provider_id=$2)",
        )
        .bind(user_id)
        .bind(provider_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;
        return Ok(!external);
    }
    let token = match provider_token_for_session(pool, settings, user_id, tenant_id, hash).await {
        Ok(token) => token,
        Err(AppError::Unauthorized | AppError::NotFound) => return Ok(false),
        Err(error) => return Err(error),
    };
    match crate::external_identity::call_bridge_as_user(
        settings,
        reqwest::Method::GET,
        "/v1/self/sessions",
        None,
        None,
        Some(&token),
    )
    .await
    {
        Ok(_) => Ok(true),
        Err(AppError::Unauthorized | AppError::Forbidden) => Ok(false),
        Err(error) => Err(error),
    }
}

fn target(path: &str, method: &str) -> Result<String, AppError> {
    let fixed = match (method, path) {
        ("GET", "sessions") => "sessions",
        ("POST", "sessions/revoke-others") => "sessions",
        ("GET", "security/capabilities") => "security/capabilities",
        ("GET", "security/profile") => "security/profile",
        ("GET", "security/linked-accounts") => "security/linked-accounts",
        ("GET", "security/mfa") => "security/mfa",
        ("POST", "security/mfa/totp/start") => "security/mfa/totp/start",
        ("POST", "security/mfa/totp/finish") => "security/mfa/totp/finish",
        ("POST", "security/mfa/recovery-codes") => "security/mfa/recovery-codes",
        ("DELETE", "security/mfa/totp") => "security/mfa/totp",
        ("GET", "security/passkeys") => "security/passkeys",
        _ => {
            let (kind, id) = path.rsplit_once('/').ok_or(AppError::NotFound)?;
            id.parse::<Uuid>().map_err(|_| AppError::NotFound)?;
            if !matches!(
                (method, kind),
                ("DELETE", "sessions")
                    | ("DELETE", "security/passkeys")
                    | ("PATCH", "security/passkeys")
            ) {
                return Err(AppError::NotFound);
            }
            return Ok(format!("/v1/self/{path}"));
        }
    };
    Ok(format!("/v1/self/{fixed}"))
}

async fn dispatch(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    body: Option<web::Json<Value>>,
) -> Result<HttpResponse, AppError> {
    let path = req.match_info().query("tail");
    let method = req.method().as_str();
    let bridge_path = target(path, method)?;
    let (access, user_id, _tenant_id, hash) =
        provider_token(&req, pool.get_ref(), settings.get_ref()).await?;
    let value = crate::external_identity::call_bridge_as_user(
        settings.get_ref(),
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| AppError::NotFound)?,
        &bridge_path,
        None,
        body.as_deref(),
        Some(&access),
    )
    .await;
    if matches!(value, Err(AppError::Unauthorized)) {
        sqlx::query("UPDATE workspace_sessions SET revoked_at=NOW() WHERE token_hash=$1")
            .bind(&hash)
            .execute(pool.get_ref())
            .await
            .map_err(|_| AppError::InternalServerError)?;
    }
    let value = value?;
    if method == "DELETE" && path.starts_with("sessions/") {
        let id: Uuid = path
            .rsplit('/')
            .next()
            .ok_or(AppError::NotFound)?
            .parse()
            .map_err(|_| AppError::NotFound)?;
        sqlx::query("UPDATE workspace_sessions SET revoked_at=NOW() WHERE user_id=$1 AND provider_session_id=$2")
            .bind(user_id).bind(id).execute(pool.get_ref()).await.map_err(|_| AppError::InternalServerError)?;
    }
    if (method == "POST" && path == "sessions/revoke-others")
        || (method == "DELETE" && path == "security/mfa/totp")
    {
        sqlx::query("UPDATE workspace_sessions SET revoked_at=NOW() WHERE user_id=$1 AND provider_session_id IS NOT NULL AND token_hash<>$2")
            .bind(user_id).bind(hash).execute(pool.get_ref()).await.map_err(|_| AppError::InternalServerError)?;
    }
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(value))
}

pub async fn get_self_security(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
) -> Result<HttpResponse, AppError> {
    dispatch(req, pool, settings, None).await
}
pub async fn mutate_self_security(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    body: Option<web::Json<Value>>,
) -> Result<HttpResponse, AppError> {
    dispatch(req, pool, settings, body).await
}
pub async fn delete_self_security(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
) -> Result<HttpResponse, AppError> {
    dispatch(req, pool, settings, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_service_routes_are_allowlisted() {
        assert_eq!(target("sessions", "GET").unwrap(), "/v1/self/sessions");
        assert_eq!(
            target("sessions/revoke-others", "POST").unwrap(),
            "/v1/self/sessions"
        );
        assert_eq!(
            target("security/mfa/totp/finish", "POST").unwrap(),
            "/v1/self/security/mfa/totp/finish"
        );
        assert!(target("../admin", "GET").is_err());
        assert!(target("security/passkeys/not-an-id", "DELETE").is_err());
        assert!(target("sessions/00000000-0000-0000-0000-000000000001", "POST").is_err());
    }

    #[test]
    fn token_session_id_reads_only_uuid_sid() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"sid":"00000000-0000-0000-0000-000000000001"}"#);
        assert_eq!(
            token_session_id(&format!("header.{payload}.signature")),
            Some(Uuid::from_u128(1))
        );
        assert_eq!(token_session_id("opaque"), None);
    }
}
