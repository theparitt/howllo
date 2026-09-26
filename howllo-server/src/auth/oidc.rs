use actix_web::{
    cookie::{Cookie, SameSite},
    get, post, web, HttpRequest, HttpResponse, Responder,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, decode_header,
    jwk::{JwkSet, KeyAlgorithm, PublicKeyUse},
    Algorithm, DecodingKey, Validation,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    customer_auth::{parse_scoped_provider_id, scoped_oidc_provider},
    identity::{resolve_user, ExternalIdentity},
    providers::{oidc_provider, OidcProviderConfig},
};
use crate::{db::DbPool, errors::AppError};

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}
#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
}
#[derive(Deserialize)]
struct IdClaims {
    sub: String,
    iss: String,
    aud: serde_json::Value,
    exp: i64,
    iat: i64,
    nonce: String,
    azp: Option<String>,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
}
#[derive(Deserialize)]
pub struct LoginQuery {
    return_to: Option<String>,
}
#[derive(Deserialize)]
pub struct CallbackQuery {
    state: Option<String>,
    code: Option<String>,
    error: Option<String>,
}
#[derive(Deserialize)]
pub struct ExchangeRequest {
    code: String,
}
#[derive(Serialize)]
pub struct ExchangeResponse {
    access_token: String,
    token_type: &'static str,
    return_to: String,
}

fn random() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
fn challenge(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}

fn state_cookie_name(provider: &str) -> String {
    format!("howllo_oidc_{provider}")
}
fn state_cookie(provider: &str, value: String) -> Cookie<'static> {
    Cookie::build(state_cookie_name(provider), value)
        .path(format!("/api/auth/callback/{provider}"))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(
            api_origin()
                .map(|url| url.starts_with("https://"))
                .unwrap_or(false),
        )
        .max_age(actix_web::cookie::time::Duration::minutes(10))
        .finish()
}

fn safe_return_to(value: Option<&str>) -> String {
    let value = value.unwrap_or("/");
    if value.starts_with('/')
        && !value.starts_with("//")
        && !value.contains('\\')
        && !value.contains('\r')
        && !value.contains('\n')
    {
        value.to_string()
    } else {
        "/".into()
    }
}

fn api_origin() -> Result<String, AppError> {
    std::env::var("HOWLLO_PUBLIC_API_URL")
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or(AppError::InternalServerError)
}
fn web_origin() -> Result<String, AppError> {
    std::env::var("HOWLLO_WEB_ORIGIN")
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or(AppError::InternalServerError)
}
fn customer_web_origin() -> Result<String, AppError> {
    std::env::var("HOWLLO_CUSTOMER_WEB_ORIGIN")
        .ok()
        .filter(|value| !value.is_empty())
        .map(Ok)
        .unwrap_or_else(web_origin)
}
fn callback_url(id: &str) -> Result<String, AppError> {
    Ok(format!(
        "{}/api/auth/callback/{id}",
        api_origin()?.trim_end_matches('/')
    ))
}
fn client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AppError::InternalServerError)
}
fn safe_endpoint(value: &str, provider: &OidcProviderConfig) -> Result<(), AppError> {
    let url = url::Url::parse(value).map_err(|_| AppError::Unauthorized)?;
    let issuer = url::Url::parse(&provider.issuer).map_err(|_| AppError::Unauthorized)?;
    let scoped = parse_scoped_provider_id(&provider.id).is_some();
    let allowed_host = if provider.id.ends_with("-google") {
        matches!(
            url.host_str(),
            Some("accounts.google.com" | "oauth2.googleapis.com" | "www.googleapis.com")
        )
    } else {
        url.host_str() == issuer.host_str()
            && url.port_or_known_default() == issuer.port_or_known_default()
    };
    if (!scoped
        && (url.scheme() == "https"
            || (url.scheme() == "http"
                && matches!(url.host_str(), Some("localhost" | "127.0.0.1")))))
        || (scoped
            && url.scheme() == "https"
            && allowed_host
            && url.username().is_empty()
            && url.password().is_none())
    {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}
async fn discovery(
    provider: &OidcProviderConfig,
    http: &reqwest::Client,
) -> Result<Discovery, AppError> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        provider.issuer.trim_end_matches('/')
    );
    let document = http
        .get(&url)
        .send()
        .await
        .map_err(|_| AppError::Unauthorized)?
        .error_for_status()
        .map_err(|_| AppError::Unauthorized)?
        .json::<Discovery>()
        .await
        .map_err(|_| AppError::Unauthorized)?;
    if document.issuer != provider.issuer {
        return Err(AppError::Unauthorized);
    }
    safe_endpoint(&document.authorization_endpoint, provider)?;
    safe_endpoint(&document.token_endpoint, provider)?;
    safe_endpoint(&document.jwks_uri, provider)?;
    Ok(document)
}

#[get("/api/auth/login/{provider}")]
pub async fn login(
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    query: web::Query<LoginQuery>,
) -> Result<impl Responder, AppError> {
    let provider = if parse_scoped_provider_id(&path).is_some() {
        scoped_oidc_provider(pool.get_ref(), &path).await?
    } else {
        oidc_provider(&path)?
    };
    let http = client()?;
    let document = discovery(&provider, &http).await?;
    let state = random();
    let nonce = random();
    let verifier = random();
    let return_to = safe_return_to(query.return_to.as_deref());
    sqlx::query("INSERT INTO auth_transactions (state_hash, provider_id, nonce, code_verifier, return_to, expires_at) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(hash(&state)).bind(&provider.id).bind(&nonce).bind(&verifier).bind(&return_to)
        .bind(Utc::now() + Duration::minutes(10)).execute(pool.get_ref()).await.map_err(storage_error)?;
    let mut target =
        url::Url::parse(&document.authorization_endpoint).map_err(|_| AppError::Unauthorized)?;
    target
        .query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &provider.client_id)
        .append_pair("redirect_uri", &callback_url(&provider.id)?)
        .append_pair("scope", &provider.scopes)
        .append_pair("state", &state)
        .append_pair("nonce", &nonce)
        .append_pair("code_challenge", &challenge(&verifier))
        .append_pair("code_challenge_method", "S256");
    tracing::info!(provider_id = %provider.id, "auth.login.started");
    Ok(HttpResponse::Found()
        .insert_header(("Location", target.as_str()))
        .insert_header(("Cache-Control", "no-store"))
        .cookie(state_cookie(&provider.id, hash(&state)))
        .finish())
}

#[get("/api/auth/callback/{provider}")]
pub async fn callback(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    query: web::Query<CallbackQuery>,
) -> Result<impl Responder, AppError> {
    let provider = if parse_scoped_provider_id(&path).is_some() {
        scoped_oidc_provider(pool.get_ref(), &path).await?
    } else {
        oidc_provider(&path)?
    };
    let state = query
        .state
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or(AppError::Unauthorized)?;
    if req
        .cookie(&state_cookie_name(&provider.id))
        .map(|cookie| cookie.value().to_string())
        != Some(hash(state))
    {
        return Err(AppError::Unauthorized);
    }
    // DELETE RETURNING makes state one-time even if the provider sends an error.
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "DELETE FROM auth_transactions WHERE state_hash = $1 AND provider_id = $2 AND expires_at > NOW() RETURNING nonce, code_verifier, return_to, provider_id"
    ).bind(hash(state)).bind(&provider.id).fetch_optional(pool.get_ref()).await.map_err(storage_error)?;
    let Some((nonce, verifier, return_to, _)) = row else {
        return Err(AppError::Unauthorized);
    };
    if query.error.is_some() {
        tracing::warn!(provider_id = %provider.id, "auth.login.failed");
        return Err(AppError::Unauthorized);
    }
    let code = query
        .code
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or(AppError::Unauthorized)?;
    let http = client()?;
    let document = discovery(&provider, &http).await?;
    let auth_method = provider.token_endpoint_auth_method.as_deref().unwrap_or(
        if provider.client_secret.as_deref().unwrap_or("").is_empty() {
            "none"
        } else {
            "client_secret_basic"
        },
    );
    let mut request = http.post(&document.token_endpoint);
    if auth_method == "client_secret_basic" {
        request = request.basic_auth(&provider.client_id, provider.client_secret.as_deref());
    }
    let redirect_uri = callback_url(&provider.id)?;
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri.as_str()),
        ("code_verifier", verifier.as_str()),
        ("client_id", provider.client_id.as_str()),
    ];
    if auth_method == "client_secret_post" {
        form.push((
            "client_secret",
            provider
                .client_secret
                .as_deref()
                .ok_or(AppError::Unauthorized)?,
        ));
    }
    let response = request
        .form(&form)
        .send()
        .await
        .map_err(|_| AppError::Unauthorized)?
        .error_for_status()
        .map_err(|_| AppError::Unauthorized)?
        .json::<TokenResponse>()
        .await
        .map_err(|_| AppError::Unauthorized)?;
    let claims = verify_id_token(&http, &document, &provider, &response.id_token, &nonce).await?;
    let email = if claims.email_verified == Some(true) {
        claims.email
    } else {
        None
    };
    let identity = ExternalIdentity {
        provider_id: provider.id.clone(),
        subject: claims.sub,
        email,
        name: claims.name,
    };
    let user = resolve_user(pool.get_ref(), &identity).await?;
    let exchange_code = random();
    let tenant_id = parse_scoped_provider_id(&provider.id).map(|(tenant_id, _)| tenant_id);
    sqlx::query("INSERT INTO auth_exchange_codes (code_hash, user_id, return_to, expires_at, tenant_id, auth_provider) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(hash(&exchange_code)).bind(user.id).bind(&return_to)
        .bind(Utc::now() + Duration::minutes(2)).bind(tenant_id).bind(&provider.id).execute(pool.get_ref()).await.map_err(storage_error)?;
    let destination = if tenant_id.is_some() {
        customer_web_origin()?
    } else {
        web_origin()?
    };
    let mut target = url::Url::parse(&format!(
        "{}/auth/callback",
        destination.trim_end_matches('/')
    ))
    .map_err(|_| AppError::InternalServerError)?;
    target
        .query_pairs_mut()
        .append_pair("exchange_code", &exchange_code);
    tracing::info!(provider_id = %provider.id, user_id = %user.id, "auth.login.succeeded");
    let mut expired = state_cookie(&provider.id, String::new());
    expired.make_removal();
    Ok(HttpResponse::Found()
        .insert_header(("Location", target.as_str()))
        .insert_header(("Cache-Control", "no-store"))
        .cookie(expired)
        .finish())
}

async fn verify_id_token(
    http: &reqwest::Client,
    document: &Discovery,
    provider: &OidcProviderConfig,
    token: &str,
    nonce: &str,
) -> Result<IdClaims, AppError> {
    let header = decode_header(token).map_err(|_| AppError::Unauthorized)?;
    if header.alg != Algorithm::RS256 {
        return Err(AppError::Unauthorized);
    }
    let kid = header.kid.as_deref().ok_or(AppError::Unauthorized)?;
    // Fetch on each callback so a rotated key is seen immediately.
    let jwks = http
        .get(&document.jwks_uri)
        .send()
        .await
        .map_err(|_| AppError::Unauthorized)?
        .error_for_status()
        .map_err(|_| AppError::Unauthorized)?
        .json::<JwkSet>()
        .await
        .map_err(|_| AppError::Unauthorized)?;
    let jwk = jwks.find(kid).ok_or(AppError::Unauthorized)?;
    if jwk
        .common
        .public_key_use
        .as_ref()
        .is_some_and(|use_| use_ != &PublicKeyUse::Signature)
        || jwk
            .common
            .key_algorithm
            .as_ref()
            .is_some_and(|alg| alg != &KeyAlgorithm::RS256)
    {
        return Err(AppError::Unauthorized);
    }
    let key = DecodingKey::from_jwk(jwk).map_err(|_| AppError::Unauthorized)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&document.issuer]);
    validation.set_audience(&[&provider.client_id]);
    validation.required_spec_claims.extend(
        ["iss", "aud", "sub", "exp", "iat"]
            .into_iter()
            .map(str::to_string),
    );
    validation.leeway = 30;
    let claims = decode::<IdClaims>(token, &key, &validation)
        .map_err(|_| AppError::Unauthorized)?
        .claims;
    if claims.nonce != nonce
        || claims.sub.trim().is_empty()
        || claims.iss != document.issuer
        || claims.exp <= Utc::now().timestamp() - 30
        || claims.iat > Utc::now().timestamp() + 30
    {
        return Err(AppError::Unauthorized);
    }
    let aud_count = claims.aud.as_array().map(|a| a.len()).unwrap_or(1);
    if (aud_count > 1 || claims.azp.is_some()) && claims.azp.as_deref() != Some(&provider.client_id)
    {
        return Err(AppError::Unauthorized);
    }
    Ok(claims)
}

#[post("/api/auth/exchange")]
pub async fn exchange(
    pool: web::Data<DbPool>,
    body: web::Json<ExchangeRequest>,
) -> Result<impl Responder, AppError> {
    let row: Option<(uuid::Uuid, String, Option<uuid::Uuid>, Option<String>)> = sqlx::query_as(
        "DELETE FROM auth_exchange_codes WHERE code_hash = $1 AND expires_at > NOW() RETURNING user_id, return_to, tenant_id, auth_provider"
    ).bind(hash(&body.code)).fetch_optional(pool.get_ref()).await.map_err(storage_error)?;
    let Some((user_id, return_to, tenant_id, provider_id)) = row else {
        return Err(AppError::Unauthorized);
    };
    let token = super::local_user::issue_scoped_account_token(
        pool.get_ref(),
        user_id,
        tenant_id,
        provider_id.as_deref().unwrap_or("oidc"),
    )
    .await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ExchangeResponse {
            access_token: token.access_token,
            token_type: "Bearer",
            return_to,
        }))
}

fn storage_error(error: sqlx::Error) -> AppError {
    tracing::error!(error = %error, "OIDC state storage error");
    AppError::InternalServerError
}

#[cfg(test)]
mod tests {
    use super::{challenge, safe_return_to};
    #[test]
    fn return_target_cannot_leave_web_origin() {
        assert_eq!(safe_return_to(Some("/board?x=1")), "/board?x=1");
        assert_eq!(safe_return_to(Some("//evil.example")), "/");
        assert_eq!(safe_return_to(Some("https://evil.example")), "/");
    }
    #[test]
    fn pkce_challenge_is_sha256_base64url() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}

#[cfg(test)]
#[path = "oidc_tests.rs"]
mod integration_tests;
