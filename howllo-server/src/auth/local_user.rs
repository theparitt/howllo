//! Local board accounts use usernames; unverified emails cannot capture invitations.
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{db::DbPool, errors::AppError, users::User};

const PREFIX: &str = "howllo_ac_";

#[derive(Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    pub username: String,
    pub recovery_code: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct RotateRecoveryRequest {
    pub current_password: String,
}

#[derive(Serialize)]
pub struct LocalAccountStatus {
    has_local_credentials: bool,
    has_recovery_code: bool,
}

#[derive(Serialize)]
pub struct AccountToken {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_code: Option<String>,
}

fn username(value: &str) -> Result<String, AppError> {
    let value = value.trim().to_ascii_lowercase();
    if !(3..=32).contains(&value.len())
        || !value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(AppError::Validation(
            "username must be 3–32 letters, digits or underscores".into(),
        ));
    }
    Ok(value)
}

fn validate_password(value: &str) -> Result<(), AppError> {
    if !(12..=128).contains(&value.len()) {
        return Err(AppError::Validation(
            "password must be 12–128 characters".into(),
        ));
    }
    Ok(())
}

fn password_hash(value: &str) -> Result<String, AppError> {
    Ok(Argon2::default()
        .hash_password(value.as_bytes(), &SaltString::generate(&mut OsRng))
        .map_err(|_| AppError::InternalServerError)?
        .to_string())
}

fn password_matches(value: &str, hash: &str) -> bool {
    PasswordHash::new(hash).ok().is_some_and(|parsed| {
        Argon2::default()
            .verify_password(value.as_bytes(), &parsed)
            .is_ok()
    })
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn new_recovery_code() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("howllo_rc_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn account_bearer(req: &HttpRequest) -> Result<&str, AppError> {
    req.headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .filter(|token| is_account_token(token))
        .ok_or(AppError::Unauthorized)
}

pub fn is_account_token(token: &str) -> bool {
    token.starts_with(PREFIX)
}

pub async fn resolve_account_token(pool: &DbPool, token: &str) -> Result<Option<User>, AppError> {
    if !is_account_token(token) {
        return Ok(None);
    }
    sqlx::query_as::<_, User>(
        "SELECT u.id, u.rooiam_subject, u.email, u.display_name, u.avatar_url, u.created_at, u.updated_at FROM account_sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = $1 AND s.revoked_at IS NULL AND s.expires_at > NOW()"
    ).bind(token_hash(token)).fetch_optional(pool).await.map_err(db_error)
}

pub async fn issue_account_token(pool: &DbPool, user_id: Uuid) -> Result<AccountToken, AppError> {
    let token = format!(
        "{PREFIX}{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    sqlx::query(
        "INSERT INTO account_sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(token_hash(&token))
    .bind(Utc::now() + Duration::days(30))
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(AccountToken {
        access_token: token,
        token_type: "Bearer",
        expires_in: Duration::days(30).num_seconds(),
        recovery_code: None,
    })
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(error = %error, "local account storage error");
    AppError::InternalServerError
}

#[post("/api/auth/local/register")]
pub async fn register(
    pool: web::Data<DbPool>,
    body: web::Json<Credentials>,
) -> Result<impl Responder, AppError> {
    if !super::providers::local_enabled() {
        return Err(AppError::NotFound);
    }
    let name = username(&body.username)?;
    validate_password(&body.password)?;
    let hash = password_hash(&body.password)?;
    let recovery_code = new_recovery_code();
    let mut tx = pool.begin().await.map_err(db_error)?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, display_name) VALUES ($1, $2, $3)")
        .bind(id)
        .bind("")
        .bind(&name)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    let inserted = sqlx::query("INSERT INTO user_identities (user_id, provider_id, subject) VALUES ($1, 'local', $2) ON CONFLICT (provider_id, subject) DO NOTHING")
        .bind(id).bind(&name).execute(&mut *tx).await.map_err(db_error)?;
    if inserted.rows_affected() == 0 {
        tx.rollback().await.map_err(db_error)?;
        return Err(AppError::Validation("username is already taken".into()));
    }
    sqlx::query(
        "INSERT INTO local_credentials (user_id, username, password_hash) VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(&name)
    .bind(&hash)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    sqlx::query("INSERT INTO local_recovery_codes (user_id, code_hash) VALUES ($1, $2)")
        .bind(id)
        .bind(token_hash(&recovery_code))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    tracing::info!(provider_id = "local", user_id = %id, "auth.login.succeeded");
    let mut account = issue_account_token(pool.get_ref(), id).await?;
    account.recovery_code = Some(recovery_code);
    Ok(HttpResponse::Created()
        .insert_header(("Cache-Control", "no-store"))
        .json(account))
}

#[post("/api/auth/local/login")]
pub async fn login(
    pool: web::Data<DbPool>,
    body: web::Json<Credentials>,
) -> Result<impl Responder, AppError> {
    if !super::providers::local_enabled() {
        return Err(AppError::NotFound);
    }
    let name = username(&body.username)?;
    let row: Option<(Uuid, String)> =
        sqlx::query_as("SELECT user_id, password_hash FROM local_credentials WHERE username = $1")
            .bind(&name)
            .fetch_optional(pool.get_ref())
            .await
            .map_err(db_error)?;
    let Some((id, hash)) = row else {
        return Err(AppError::Unauthorized);
    };
    if !password_matches(&body.password, &hash) {
        tracing::warn!(provider_id = "local", "auth.login.failed");
        return Err(AppError::Unauthorized);
    }
    tracing::info!(provider_id = "local", user_id = %id, "auth.login.succeeded");
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(issue_account_token(pool.get_ref(), id).await?))
}

#[post("/api/auth/local/change-password")]
pub async fn change_password(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    body: web::Json<ChangePasswordRequest>,
) -> Result<impl Responder, AppError> {
    if !super::providers::local_enabled() {
        return Err(AppError::NotFound);
    }
    validate_password(&body.new_password)?;
    let token = account_bearer(&req)?;
    let user = resolve_account_token(pool.get_ref(), token)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let old_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM local_credentials WHERE user_id = $1")
            .bind(user.id)
            .fetch_optional(pool.get_ref())
            .await
            .map_err(db_error)?
            .ok_or(AppError::Unauthorized)?;
    if !password_matches(&body.current_password, &old_hash) {
        tracing::warn!(provider_id = "local", "auth.password_change.failed");
        return Err(AppError::Unauthorized);
    }
    if body.current_password == body.new_password {
        return Err(AppError::Validation(
            "new password must be different".into(),
        ));
    }
    let new_hash = password_hash(&body.new_password)?;
    let mut tx = pool.begin().await.map_err(db_error)?;
    let updated = sqlx::query(
        "UPDATE local_credentials SET password_hash = $1 WHERE user_id = $2 AND password_hash = $3",
    )
    .bind(new_hash)
    .bind(user.id)
    .bind(old_hash)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Unauthorized);
    }
    revoke_user_sessions(&mut tx, user.id).await?;
    tx.commit().await.map_err(db_error)?;
    tracing::info!(provider_id = "local", user_id = %user.id, "auth.password_change.succeeded");
    Ok(HttpResponse::NoContent()
        .insert_header(("Cache-Control", "no-store"))
        .finish())
}

#[get("/api/auth/local/status")]
pub async fn status(req: HttpRequest, pool: web::Data<DbPool>) -> Result<impl Responder, AppError> {
    let user = resolve_account_token(pool.get_ref(), account_bearer(&req)?)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let row: Option<(bool, bool)> = sqlx::query_as(
        "SELECT TRUE, EXISTS(SELECT 1 FROM local_recovery_codes r WHERE r.user_id = c.user_id) FROM local_credentials c WHERE c.user_id = $1",
    )
    .bind(user.id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(db_error)?;
    Ok(HttpResponse::Ok().json(LocalAccountStatus {
        has_local_credentials: row.is_some(),
        has_recovery_code: row.is_some_and(|(_, has_code)| has_code),
    }))
}

#[post("/api/auth/local/rotate-recovery-code")]
pub async fn rotate_recovery_code(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    body: web::Json<RotateRecoveryRequest>,
) -> Result<impl Responder, AppError> {
    if !super::providers::local_enabled() {
        return Err(AppError::NotFound);
    }
    let user = resolve_account_token(pool.get_ref(), account_bearer(&req)?)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let old_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM local_credentials WHERE user_id = $1")
            .bind(user.id)
            .fetch_optional(pool.get_ref())
            .await
            .map_err(db_error)?
            .ok_or(AppError::Unauthorized)?;
    if !password_matches(&body.current_password, &old_hash) {
        return Err(AppError::Unauthorized);
    }
    let code = new_recovery_code();
    sqlx::query("INSERT INTO local_recovery_codes (user_id, code_hash) VALUES ($1, $2) ON CONFLICT (user_id) DO UPDATE SET code_hash = EXCLUDED.code_hash, created_at = NOW()")
        .bind(user.id)
        .bind(token_hash(&code))
        .execute(pool.get_ref())
        .await
        .map_err(db_error)?;
    tracing::info!(provider_id = "local", user_id = %user.id, "auth.recovery_code.rotated");
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(serde_json::json!({ "recovery_code": code })))
}

#[post("/api/auth/local/reset-password")]
pub async fn reset_password(
    pool: web::Data<DbPool>,
    body: web::Json<ResetPasswordRequest>,
) -> Result<impl Responder, AppError> {
    if !super::providers::local_enabled() {
        return Err(AppError::NotFound);
    }
    validate_password(&body.new_password)?;
    let name = username(&body.username)?;
    let new_hash = password_hash(&body.new_password)?;
    let replacement_code = new_recovery_code();
    let mut tx = pool.begin().await.map_err(db_error)?;
    let user_id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE local_recovery_codes r SET code_hash = $1, created_at = NOW() FROM local_credentials c WHERE r.user_id = c.user_id AND c.username = $2 AND r.code_hash = $3 RETURNING r.user_id",
    )
    .bind(token_hash(&replacement_code))
    .bind(name)
    .bind(token_hash(&body.recovery_code))
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;
    let Some(user_id) = user_id else {
        return Err(AppError::Unauthorized);
    };
    sqlx::query("UPDATE local_credentials SET password_hash = $1 WHERE user_id = $2")
        .bind(new_hash)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    revoke_user_sessions(&mut tx, user_id).await?;
    tx.commit().await.map_err(db_error)?;
    tracing::info!(provider_id = "local", user_id = %user_id, "auth.password_reset.succeeded");
    let mut account = issue_account_token(pool.get_ref(), user_id).await?;
    account.recovery_code = Some(replacement_code);
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(account))
}

async fn revoke_user_sessions(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE account_sessions SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query("UPDATE workspace_sessions SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    Ok(())
}

#[post("/api/auth/logout")]
pub async fn logout(req: HttpRequest, pool: web::Data<DbPool>) -> Result<impl Responder, AppError> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;
    if !is_account_token(token) {
        return Err(AppError::Unauthorized);
    }
    sqlx::query("UPDATE account_sessions SET revoked_at = NOW() WHERE token_hash = $1")
        .bind(token_hash(token))
        .execute(pool.get_ref())
        .await
        .map_err(db_error)?;
    tracing::info!(provider_id = "local", "auth.logout");
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::username;
    use crate::{
        db,
        http::test_support::{lock_test_db, read_json, reset_db, test_settings},
        startup,
    };
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;
    #[actix_web::test]
    async fn usernames_are_normalized_and_constrained() {
        assert_eq!(username(" Alice_42 ").unwrap(), "alice_42");
        assert!(username("a").is_err());
        assert!(username("alice@example.com").is_err());
    }

    #[actix_web::test]
    async fn local_account_can_create_workspace_and_logout() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings))
                .configure(startup::configure),
        )
        .await;
        let credentials = json!({"username":"Alice_42","password":"this-is-a-long-local-password"});
        let created = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/register")
                .set_json(&credentials)
                .to_request(),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let token = read_json(created).await["access_token"]
            .as_str()
            .unwrap()
            .to_string();
        let duplicate = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/register")
                .set_json(&credentials)
                .to_request(),
        )
        .await;
        assert_eq!(duplicate.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let wrong = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/login")
                .set_json(json!({"username":"alice_42","password":"wrong-password"}))
                .to_request(),
        )
        .await;
        assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
        let bearer = format!("Bearer {token}");
        let workspace = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/admin/tenants")
                .insert_header(("Authorization", bearer.clone()))
                .set_json(json!({"name":"Local Team"}))
                .to_request(),
        )
        .await;
        assert_eq!(workspace.status(), StatusCode::CREATED);
        let slug = read_json(workspace).await["slug"]
            .as_str()
            .unwrap()
            .to_string();
        let session = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/workspace-session")
                .insert_header(("Authorization", bearer.clone()))
                .set_json(json!({"tenant_slug":slug}))
                .to_request(),
        )
        .await;
        assert_eq!(session.status(), StatusCode::CREATED);
        let logout = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/logout")
                .insert_header(("Authorization", bearer.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(logout.status(), StatusCode::NO_CONTENT);
        let me = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/me")
                .insert_header(("Authorization", bearer))
                .to_request(),
        )
        .await;
        assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn recovery_code_is_one_time_and_password_changes_revoke_sessions() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings))
                .configure(startup::configure),
        )
        .await;
        let created = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/register")
                .set_json(json!({"username":"recover_me","password":"first-strong-password"}))
                .to_request(),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        assert_eq!(created.headers().get("Cache-Control").unwrap(), "no-store");
        let created = read_json(created).await;
        let first_code = created["recovery_code"].as_str().unwrap();
        let first_token = created["access_token"].as_str().unwrap();
        assert!(first_code.starts_with("howllo_rc_"));
        let status = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/auth/local/status")
                .insert_header(("Authorization", format!("Bearer {first_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(read_json(status).await["has_recovery_code"], true);
        let workspace = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/admin/tenants")
                .insert_header(("Authorization", format!("Bearer {first_token}")))
                .set_json(json!({"name":"Recovery Team"}))
                .to_request(),
        )
        .await;
        assert_eq!(workspace.status(), StatusCode::CREATED);
        let slug = read_json(workspace).await["slug"]
            .as_str()
            .unwrap()
            .to_string();
        let workspace_session = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/workspace-session")
                .insert_header(("Authorization", format!("Bearer {first_token}")))
                .set_json(json!({"tenant_slug":slug}))
                .to_request(),
        )
        .await;
        assert_eq!(workspace_session.status(), StatusCode::CREATED);
        let workspace_token = read_json(workspace_session).await["session_token"]
            .as_str()
            .unwrap()
            .to_string();
        let wrong_rotation = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/rotate-recovery-code")
                .insert_header(("Authorization", format!("Bearer {first_token}")))
                .set_json(json!({"current_password":"wrong"}))
                .to_request(),
        )
        .await;
        assert_eq!(wrong_rotation.status(), StatusCode::UNAUTHORIZED);
        let rotated = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/rotate-recovery-code")
                .insert_header(("Authorization", format!("Bearer {first_token}")))
                .set_json(json!({"current_password":"first-strong-password"}))
                .to_request(),
        )
        .await;
        assert_eq!(rotated.status(), StatusCode::OK);
        let second_code = read_json(rotated).await["recovery_code"]
            .as_str()
            .unwrap()
            .to_string();
        assert_ne!(first_code, second_code);
        let stale = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/reset-password")
                .set_json(json!({"username":"recover_me","recovery_code":first_code,"new_password":"second-strong-password"}))
                .to_request(),
        )
        .await;
        assert_eq!(stale.status(), StatusCode::UNAUTHORIZED);
        let reset = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/reset-password")
                .set_json(json!({"username":"recover_me","recovery_code":second_code,"new_password":"second-strong-password"}))
                .to_request(),
        )
        .await;
        assert_eq!(reset.status(), StatusCode::OK);
        let reset = read_json(reset).await;
        let third_code = reset["recovery_code"].as_str().unwrap();
        let second_token = reset["access_token"].as_str().unwrap();
        assert_ne!(third_code, second_code);
        let replay = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/reset-password")
                .set_json(json!({"username":"recover_me","recovery_code":second_code,"new_password":"attacker-password-123"}))
                .to_request(),
        )
        .await;
        assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
        let old_session = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/me")
                .insert_header(("Authorization", format!("Bearer {first_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(old_session.status(), StatusCode::UNAUTHORIZED);
        let old_workspace_session = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/me")
                .insert_header(("Authorization", format!("Bearer {workspace_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(old_workspace_session.status(), StatusCode::UNAUTHORIZED);
        let wrong_change = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/change-password")
                .insert_header(("Authorization", format!("Bearer {second_token}")))
                .set_json(
                    json!({"current_password":"wrong","new_password":"third-strong-password"}),
                )
                .to_request(),
        )
        .await;
        assert_eq!(wrong_change.status(), StatusCode::UNAUTHORIZED);
        let changed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/change-password")
                .insert_header(("Authorization", format!("Bearer {second_token}")))
                .set_json(json!({"current_password":"second-strong-password","new_password":"third-strong-password"}))
                .to_request(),
        )
        .await;
        assert_eq!(changed.status(), StatusCode::NO_CONTENT);
        let old_login = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/login")
                .set_json(json!({"username":"recover_me","password":"second-strong-password"}))
                .to_request(),
        )
        .await;
        assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);
        let new_login = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/local/login")
                .set_json(json!({"username":"recover_me","password":"third-strong-password"}))
                .to_request(),
        )
        .await;
        assert_eq!(new_login.status(), StatusCode::OK);
        assert!(read_json(new_login).await.get("recovery_code").is_none());
    }
}
