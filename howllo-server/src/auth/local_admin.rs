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

use crate::auth::RooiamClaims;
use crate::config::Settings;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::users::User;
use actix_web::{get, post, web, HttpResponse, Responder};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

/// Stable JWT subject + user identity for the local admin.
const LOCAL_ADMIN_SUBJECT: &str = "local-admin";
const LOCAL_ADMIN_EMAIL: &str = "admin@howllo.local";
const LOCAL_ADMIN_NAME: &str = "Administrator";
const TOKEN_TTL_HOURS: i64 = 8;
const MIN_PASSWORD_LEN: usize = 8;

pub fn is_local_admin_user(user: &User) -> bool {
    user.rooiam_subject == LOCAL_ADMIN_SUBJECT
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

fn mint_token(secret: &str) -> Result<(String, i64), AppError> {
    let expires_in = Duration::hours(TOKEN_TTL_HOURS);
    let claims = RooiamClaims {
        sub: LOCAL_ADMIN_SUBJECT.to_string(),
        exp: (Utc::now() + expires_in).timestamp() as usize,
        email: Some(LOCAL_ADMIN_EMAIL.to_string()),
        name: Some(LOCAL_ADMIN_NAME.to_string()),
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

    sqlx::query!(
        r#"
        INSERT INTO memberships (tenant_id, user_id, role)
        SELECT t.id, $1, 'owner' FROM tenants t
        ON CONFLICT (tenant_id, user_id)
        DO UPDATE SET role = 'owner'
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

    let (token, expires_in) = mint_token(&settings.rooiam_jwt_secret)?;
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

    let (token, expires_in) = mint_token(&settings.rooiam_jwt_secret)?;
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
