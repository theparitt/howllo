//! Local single-admin authentication.
//!
//! The admin panel does not depend on an external identity provider. On first
//! run the operator sets a password, gated by `HOWLLO_ADMIN_BOOTSTRAP_KEY`. The
//! password is stored only as an Argon2id hash. A successful login mints a
//! short-lived JWT signed with `HOWLLO_JWT_SECRET` — the exact token the
//! existing [`AuthenticatedUser`](crate::auth::AuthenticatedUser) extractor and
//! `require_admin` path already validate. No authorization checks are relaxed:
//! the minted token resolves to a real user that holds an `owner` membership,
//! and every `/api/admin/*` handler still verifies role server-side.
//!
//! Security notes:
//! - The bootstrap key only works while no password is set; after setup it is
//!   inert. Setup is rejected entirely if `HOWLLO_ADMIN_BOOTSTRAP_KEY` is unset.
//! - Tokens are short-lived (8h) so a leaked token expires on its own.
//! - This is intended for a single-admin, localhost-bound deployment.

use crate::config::Settings;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::users::User;
use actix_web::{get, post, web, HttpResponse, Responder};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct AdminClaims {
    sub: String,
    exp: usize,
    token_use: String,
}

/// Stable JWT subject + user identity for the local admin.
const LOCAL_ADMIN_SUBJECT: &str = "local-admin";
const LOCAL_ADMIN_EMAIL: &str = "admin@howllo.local";
const LOCAL_ADMIN_NAME: &str = "Administrator";
const TOKEN_TTL_HOURS: i64 = 8;
const MIN_PASSWORD_LEN: usize = 12;
const MAX_LOGIN_ATTEMPTS_PER_MINUTE: i32 = 30;

async fn limit_admin_auth(pool: &DbPool) -> Result<(), AppError> {
    // One operator identity: use one PostgreSQL counter shared by all API
    // instances. A caller cannot bypass it with proxy headers.
    let attempts: i32 = sqlx::query_scalar(
        "INSERT INTO admin_auth_rate_limit (id,window_start,attempts)
         VALUES (TRUE,date_trunc('minute',clock_timestamp()),1)
         ON CONFLICT (id) DO UPDATE SET
           attempts = CASE WHEN admin_auth_rate_limit.window_start < date_trunc('minute',clock_timestamp())
                           THEN 1 ELSE admin_auth_rate_limit.attempts + 1 END,
           window_start = date_trunc('minute',clock_timestamp())
         RETURNING attempts",
    )
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(%error, "failed to apply admin authentication limit");
        AppError::InternalServerError
    })?;
    if attempts > MAX_LOGIN_ATTEMPTS_PER_MINUTE {
        return Err(AppError::TooManyRequests(
            "Too many admin sign-in attempts. Try again in a minute.".into(),
        ));
    }
    Ok(())
}

pub fn is_local_admin_user(user: &User) -> bool {
    user.rooiam_subject.as_deref() == Some(LOCAL_ADMIN_SUBJECT)
}

pub async fn resolve_admin_token(
    pool: &DbPool,
    settings: &Settings,
    token: &str,
) -> Result<Option<User>, AppError> {
    let validation = Validation::new(Algorithm::HS256);
    let Ok(data) = decode::<AdminClaims>(
        token,
        &DecodingKey::from_secret(settings.admin_jwt_secret.as_bytes()),
        &validation,
    ) else {
        return Ok(None);
    };
    if data.claims.sub != LOCAL_ADMIN_SUBJECT || data.claims.token_use != "howllo-admin" {
        return Ok(None);
    }
    sqlx::query_as::<_, User>("SELECT id, rooiam_subject, email, display_name, avatar_url, created_at, updated_at FROM users WHERE rooiam_subject = $1")
        .bind(LOCAL_ADMIN_SUBJECT).fetch_optional(pool).await.map_err(|error| {
            tracing::error!(error = %error, "error resolving local admin token");
            AppError::InternalServerError
        })
}

#[derive(Serialize)]
pub struct AuthStateResponse {
    /// Whether a password has been set. `false` means the setup flow applies.
    pub bootstrapped: bool,
    /// Whether the server has a bootstrap key configured at all. If `false`,
    /// setup is impossible and the operator must set `HOWLLO_ADMIN_BOOTSTRAP_KEY`.
    pub setup_available: bool,
}

#[derive(Deserialize)]
pub struct SetupRequest {
    pub bootstrap_key: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: TokenUser,
}

#[derive(Serialize)]
pub struct TokenUser {
    pub name: &'static str,
    pub email: &'static str,
}

async fn is_bootstrapped(pool: &DbPool) -> Result<bool, AppError> {
    let row = sqlx::query!("SELECT EXISTS(SELECT 1 FROM admin_credentials) AS \"exists!\"")
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to read admin_credentials");
            AppError::InternalServerError
        })?;
    Ok(row.exists)
}

fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| {
            tracing::error!(error = %e, "failed to hash admin password");
            AppError::InternalServerError
        })
}

fn verify_password(password: &str, stored_hash: &str) -> bool {
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(e) => {
            tracing::error!(error = %e, "stored admin password hash is invalid");
            false
        }
    }
}

pub async fn verify_operator_password(pool: &DbPool, password: &str) -> Result<bool, AppError> {
    let stored: Option<String> =
        sqlx::query_scalar("SELECT password_hash FROM admin_credentials WHERE id = TRUE")
            .fetch_optional(pool)
            .await
            .map_err(|error| {
                tracing::error!(%error, "failed to verify operator password");
                AppError::InternalServerError
            })?;
    Ok(stored.is_some_and(|hash| verify_password(password, &hash)))
}

fn mint_token(secret: &str) -> Result<(String, i64), AppError> {
    let expires_in = Duration::hours(TOKEN_TTL_HOURS);
    let claims = AdminClaims {
        sub: LOCAL_ADMIN_SUBJECT.to_string(),
        exp: (Utc::now() + expires_in).timestamp() as usize,
        token_use: "howllo-admin".to_string(),
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| {
        tracing::error!(error = %e, "failed to mint admin token");
        AppError::InternalServerError
    })?;
    Ok((token, expires_in.num_seconds()))
}

/// Ensure the local admin user exists and holds an `owner` membership on every
/// tenant. Called on setup and on each login so tenants created later are
/// always covered. Idempotent.
async fn ensure_admin_memberships(pool: &DbPool) -> Result<(), AppError> {
    let user = sqlx::query!(
        r#"
        INSERT INTO users (rooiam_subject, email, display_name)
        VALUES ($1, $2, $3)
        ON CONFLICT (rooiam_subject)
        DO UPDATE SET email = EXCLUDED.email, display_name = EXCLUDED.display_name
        RETURNING id
        "#,
        LOCAL_ADMIN_SUBJECT,
        LOCAL_ADMIN_EMAIL,
        LOCAL_ADMIN_NAME,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to ensure local admin user");
        AppError::InternalServerError
    })?;

    sqlx::query("INSERT INTO user_identities (user_id, provider_id, subject, email) VALUES ($1, 'local-admin', $2, $3) ON CONFLICT (provider_id, subject) DO NOTHING")
        .bind(user.id).bind(LOCAL_ADMIN_SUBJECT).bind(LOCAL_ADMIN_EMAIL)
        .execute(pool).await.map_err(|error| {
            tracing::error!(error = %error, "failed to ensure local admin identity");
            AppError::InternalServerError
        })?;

    sqlx::query!(
        r#"
        INSERT INTO memberships (tenant_id, user_id, role)
        SELECT t.id, $1, 'owner' FROM tenants t
        ON CONFLICT (tenant_id, user_id)
        DO UPDATE SET role = 'owner', public_participant = FALSE
        "#,
        user.id,
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to grant local admin memberships");
        AppError::InternalServerError
    })?;

    Ok(())
}

#[get("/api/admin/auth/state")]
pub async fn auth_state(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
) -> Result<impl Responder, AppError> {
    let bootstrapped = is_bootstrapped(pool.get_ref()).await?;
    Ok(HttpResponse::Ok().json(AuthStateResponse {
        bootstrapped,
        setup_available: settings.admin_bootstrap_key.is_some(),
    }))
}

#[post("/api/admin/auth/setup")]
pub async fn auth_setup(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    body: web::Json<SetupRequest>,
) -> Result<impl Responder, AppError> {
    let configured_key = settings.admin_bootstrap_key.as_deref().ok_or_else(|| {
        tracing::error!("admin setup attempted but HOWLLO_ADMIN_BOOTSTRAP_KEY is not set");
        AppError::Forbidden
    })?;

    if is_bootstrapped(pool.get_ref()).await? {
        // Password already set; the bootstrap key no longer grants access.
        return Err(AppError::Forbidden);
    }
    limit_admin_auth(pool.get_ref()).await?;

    // Constant-ish comparison; bootstrap key is single-use so timing is moot.
    if body.bootstrap_key.trim() != configured_key {
        tracing::warn!("admin setup rejected: bootstrap key mismatch");
        return Err(AppError::Unauthorized);
    }

    if body.password.len() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }

    let hash = hash_password(&body.password)?;

    sqlx::query!(
        "INSERT INTO admin_credentials (id, password_hash) VALUES (TRUE, $1)",
        hash,
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to store admin credentials");
        AppError::InternalServerError
    })?;

    ensure_admin_memberships(pool.get_ref()).await?;

    let (token, expires_in) = mint_token(&settings.admin_jwt_secret)?;
    tracing::info!("local admin bootstrapped");
    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token: token,
        token_type: "Bearer",
        expires_in,
        user: TokenUser {
            name: LOCAL_ADMIN_NAME,
            email: LOCAL_ADMIN_EMAIL,
        },
    }))
}

#[post("/api/admin/auth/login")]
pub async fn auth_login(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    body: web::Json<LoginRequest>,
) -> Result<impl Responder, AppError> {
    limit_admin_auth(pool.get_ref()).await?;
    let stored = sqlx::query!("SELECT password_hash FROM admin_credentials LIMIT 1")
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to read admin credentials");
            AppError::InternalServerError
        })?;

    let stored = match stored {
        Some(row) => row.password_hash,
        // Not bootstrapped yet — no password to check against.
        None => return Err(AppError::Unauthorized),
    };

    if !verify_password(&body.password, &stored) {
        tracing::warn!("admin login rejected: bad password");
        return Err(AppError::Unauthorized);
    }

    // Re-assert memberships in case tenants were created after bootstrap.
    ensure_admin_memberships(pool.get_ref()).await?;

    let (token, expires_in) = mint_token(&settings.admin_jwt_secret)?;
    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token: token,
        token_type: "Bearer",
        expires_in,
        user: TokenUser {
            name: LOCAL_ADMIN_NAME,
            email: LOCAL_ADMIN_EMAIL,
        },
    }))
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;

    use crate::{
        db,
        http::test_support::{lock_test_db, test_settings},
        startup,
    };

    #[actix_web::test]
    async fn admin_login_limit_is_shared_and_resets_after_window() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        sqlx::query("INSERT INTO admin_auth_rate_limit (id,window_start,attempts) VALUES (TRUE,date_trunc('minute',clock_timestamp()),30) ON CONFLICT (id) DO UPDATE SET window_start=EXCLUDED.window_start,attempts=EXCLUDED.attempts")
            .execute(&pool).await.unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings))
                .configure(startup::configure),
        )
        .await;
        let denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/admin/auth/login")
                .set_json(json!({"password":"incorrect"}))
                .to_request(),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::TOO_MANY_REQUESTS);
        sqlx::query("UPDATE admin_auth_rate_limit SET window_start=NOW()-INTERVAL '2 minutes'")
            .execute(&pool)
            .await
            .unwrap();
        let after_window = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/admin/auth/login")
                .set_json(json!({"password":"incorrect"}))
                .to_request(),
        )
        .await;
        assert_eq!(after_window.status(), StatusCode::UNAUTHORIZED);
        sqlx::query("DELETE FROM admin_auth_rate_limit")
            .execute(&pool)
            .await
            .unwrap();
    }
}
