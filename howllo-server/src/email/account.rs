use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::{local_user, AuthenticatedUser},
    db::DbPool,
    errors::AppError,
};

use super::{
    db_error,
    delivery::{enqueue, EmailRequest, QueueResult},
    mailbox, message_hash, platform,
};

#[derive(Deserialize)]
pub struct RequestVerification {
    email: String,
}

#[derive(Deserialize)]
pub struct ConfirmVerification {
    token: String,
}

#[derive(Deserialize)]
pub struct RequestReset {
    email: String,
}

#[derive(Deserialize)]
pub struct ConfirmReset {
    token: String,
    new_password: String,
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

async fn enabled(pool: &DbPool) -> Result<(), AppError> {
    if platform(pool).await?.status == "enabled" {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

#[post("/api/me/email-verification/request")]
pub async fn request_verification(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<RequestVerification>,
) -> Result<impl Responder, AppError> {
    enabled(pool.get_ref()).await?;
    mailbox(&body.email, None)?;
    let email = body.email.trim().to_ascii_lowercase();
    let already_owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_verified_emails WHERE email=$1 AND user_id<>$2)",
    )
    .bind(&email)
    .bind(auth.0.id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(db_error)?;
    if already_owned {
        return Err(AppError::Validation(
            "This email address is already in use.".into(),
        ));
    }
    let attempts: i32 = sqlx::query_scalar("INSERT INTO local_auth_rate_limits(scope,identity_hash,window_start,attempts) VALUES('email-verification-user',$1,NOW(),1) ON CONFLICT(scope,identity_hash) DO UPDATE SET attempts=CASE WHEN local_auth_rate_limits.window_start<NOW()-INTERVAL '1 hour' THEN 1 ELSE local_auth_rate_limits.attempts+1 END,window_start=CASE WHEN local_auth_rate_limits.window_start<NOW()-INTERVAL '1 hour' THEN NOW() ELSE local_auth_rate_limits.window_start END RETURNING attempts")
        .bind(message_hash(&auth.0.id.to_string())).fetch_one(pool.get_ref()).await.map_err(db_error)?;
    if attempts > 3 {
        return Err(AppError::TooManyRequests(
            "Too many verification requests. Try again later.".into(),
        ));
    }
    let recent: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM email_verification_tokens WHERE user_id=$1 AND created_at>NOW()-INTERVAL '5 minutes')")
        .bind(auth.0.id).fetch_one(pool.get_ref()).await.map_err(db_error)?;
    if recent {
        return Err(AppError::TooManyRequests(
            "Wait five minutes before requesting another verification email.".into(),
        ));
    }
    let token = new_token();
    let hash = message_hash(&token);
    sqlx::query("INSERT INTO email_verification_tokens(user_id,email,token_hash,expires_at) VALUES($1,$2,$3,NOW()+INTERVAL '20 minutes') ON CONFLICT(user_id) DO UPDATE SET email=EXCLUDED.email,token_hash=EXCLUDED.token_hash,expires_at=EXCLUDED.expires_at,created_at=NOW()")
        .bind(auth.0.id).bind(&email).bind(&hash).execute(pool.get_ref()).await.map_err(db_error)?;
    let body = format!("Your Howllo email verification code is:\n\n{token}\n\nIt expires in 20 minutes. If you did not request this, ignore it.");
    let outcome = enqueue(
        pool.get_ref(),
        EmailRequest {
            tenant_id: None,
            user_id: Some(auth.0.id),
            recipient: &email,
            kind: "system",
            category: "email_verification",
            subject: "Verify your Howllo email",
            body: &body,
            dedupe_key: Some(format!("verify:{}:{}", auth.0.id, hash)),
            delay_seconds: 0,
        },
    )
    .await;
    if outcome.is_err() {
        sqlx::query("DELETE FROM email_verification_tokens WHERE user_id=$1 AND token_hash=$2")
            .bind(auth.0.id)
            .bind(&hash)
            .execute(pool.get_ref())
            .await
            .map_err(db_error)?;
    }
    let outcome = outcome?;
    if !matches!(outcome, QueueResult::Queued(_)) {
        sqlx::query("DELETE FROM email_verification_tokens WHERE user_id=$1 AND token_hash=$2")
            .bind(auth.0.id)
            .bind(hash)
            .execute(pool.get_ref())
            .await
            .map_err(db_error)?;
        return Err(AppError::TooManyRequests(
            "Email cannot be sent right now. Please try later.".into(),
        ));
    }
    Ok(HttpResponse::Accepted()
        .insert_header(("Cache-Control", "no-store"))
        .finish())
}

#[post("/api/me/email-verification/confirm")]
pub async fn confirm_verification(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<ConfirmVerification>,
) -> Result<impl Responder, AppError> {
    enabled(pool.get_ref()).await?;
    if body.token.len() > 128 {
        return Err(AppError::Validation("Invalid verification code.".into()));
    }
    let mut tx = pool.begin().await.map_err(db_error)?;
    let email: Option<String> = sqlx::query_scalar("DELETE FROM email_verification_tokens WHERE user_id=$1 AND token_hash=$2 AND expires_at>NOW() RETURNING email")
        .bind(auth.0.id).bind(message_hash(&body.token)).fetch_optional(&mut *tx).await.map_err(db_error)?;
    let email = email.ok_or(AppError::Validation(
        "Verification code is invalid or expired.".into(),
    ))?;
    sqlx::query("INSERT INTO user_verified_emails(user_id,email) VALUES($1,$2) ON CONFLICT(user_id) DO UPDATE SET email=EXCLUDED.email,verified_at=NOW()")
        .bind(auth.0.id).bind(&email).execute(&mut *tx).await.map_err(|error| {
            if let sqlx::Error::Database(ref db) = error { if db.is_unique_violation() { return AppError::Validation("This email address is already in use.".into()); } }
            db_error(error)
        })?;
    // An old inbox must not retain a usable reset token after an address change.
    sqlx::query(
        "UPDATE local_email_reset_tokens SET used_at=NOW() WHERE user_id=$1 AND used_at IS NULL",
    )
    .bind(auth.0.id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    sqlx::query("UPDATE email_messages SET status='cancelled',body='' WHERE user_id=$1 AND category='password_reset' AND status='queued'")
        .bind(auth.0.id).execute(&mut *tx).await.map_err(db_error)?;
    sqlx::query("UPDATE email_messages SET status='cancelled',body='' WHERE user_id=$1 AND category='email_verification' AND status='queued'")
        .bind(auth.0.id).execute(&mut *tx).await.map_err(db_error)?;
    sqlx::query("INSERT INTO email_events(actor_user_id,event_type) VALUES($1,'address_verified')")
        .bind(auth.0.id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(serde_json::json!({"verified_email":email})))
}

async fn limit_reset(pool: &DbPool, req: &HttpRequest, email: &str) -> Result<(), AppError> {
    for (scope, identity, max) in [
        (
            "email-reset:ip",
            crate::http::client_ip(req)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "unknown".into()),
            10,
        ),
        ("email-reset:address", email.to_owned(), 3),
    ] {
        let attempts: i32 = sqlx::query_scalar("INSERT INTO local_auth_rate_limits(scope,identity_hash,window_start,attempts) VALUES($1,$2,NOW(),1) ON CONFLICT(scope,identity_hash) DO UPDATE SET attempts=CASE WHEN local_auth_rate_limits.window_start<NOW()-INTERVAL '1 hour' THEN 1 ELSE local_auth_rate_limits.attempts+1 END,window_start=CASE WHEN local_auth_rate_limits.window_start<NOW()-INTERVAL '1 hour' THEN NOW() ELSE local_auth_rate_limits.window_start END RETURNING attempts")
            .bind(scope).bind(message_hash(&identity)).fetch_one(pool).await.map_err(db_error)?;
        if attempts > max {
            return Err(AppError::TooManyRequests(
                "Too many reset requests. Please try later.".into(),
            ));
        }
    }
    Ok(())
}

#[post("/api/auth/local/request-email-reset")]
pub async fn request_email_reset(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    body: web::Json<RequestReset>,
) -> Result<impl Responder, AppError> {
    enabled(pool.get_ref()).await?;
    let email = body.email.trim().to_ascii_lowercase();
    mailbox(&email, None)?;
    limit_reset(pool.get_ref(), &req, &email).await?;
    let user_id: Option<Uuid> = sqlx::query_scalar("SELECT v.user_id FROM user_verified_emails v JOIN local_credentials c ON c.user_id=v.user_id WHERE v.email=$1")
        .bind(&email).fetch_optional(pool.get_ref()).await.map_err(db_error)?;
    if let Some(user_id) = user_id {
        let token = new_token();
        let hash = message_hash(&token);
        sqlx::query("INSERT INTO local_email_reset_tokens(user_id,token_hash,expires_at) VALUES($1,$2,NOW()+INTERVAL '20 minutes')")
            .bind(user_id).bind(&hash).execute(pool.get_ref()).await.map_err(db_error)?;
        let web_url = std::env::var("HOWLLO_CUSTOMER_WEB_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:7703".into());
        let message = format!("Use this one-time code to reset your Howllo password:\n\n{token}\n\nOpen {web_url} and choose email reset. This code expires in 20 minutes. If you did not request it, ignore this email.");
        let result = enqueue(
            pool.get_ref(),
            EmailRequest {
                tenant_id: None,
                user_id: Some(user_id),
                recipient: &email,
                kind: "system",
                category: "password_reset",
                subject: "Reset your Howllo password",
                body: &message,
                dedupe_key: Some(format!("reset:{hash}")),
                delay_seconds: 0,
            },
        )
        .await;
        if !matches!(result, Ok(QueueResult::Queued(_))) {
            sqlx::query("DELETE FROM local_email_reset_tokens WHERE token_hash=$1")
                .bind(hash)
                .execute(pool.get_ref())
                .await
                .map_err(db_error)?;
        }
    }
    // Existing and unknown accounts receive the same response; never expose address ownership.
    Ok(HttpResponse::Accepted()
        .insert_header(("Cache-Control", "no-store"))
        .finish())
}

#[post("/api/auth/local/confirm-email-reset")]
pub async fn confirm_email_reset(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    body: web::Json<ConfirmReset>,
) -> Result<impl Responder, AppError> {
    enabled(pool.get_ref()).await?;
    local_user::validate_password(&body.new_password)?;
    if body.token.len() > 128 {
        return Err(AppError::Unauthorized);
    }
    limit_reset(pool.get_ref(), &req, &message_hash(&body.token)).await?;
    let new_hash = local_user::password_hash(&body.new_password)?;
    let recovery_code = local_user::new_recovery_code();
    let mut tx = pool.begin().await.map_err(db_error)?;
    let user_id: Option<Uuid> = sqlx::query_scalar("UPDATE local_email_reset_tokens SET used_at=NOW() WHERE token_hash=$1 AND used_at IS NULL AND expires_at>NOW() RETURNING user_id")
        .bind(message_hash(&body.token)).fetch_optional(&mut *tx).await.map_err(db_error)?;
    let user_id = user_id.ok_or(AppError::Unauthorized)?;
    sqlx::query("UPDATE local_credentials SET password_hash=$1 WHERE user_id=$2")
        .bind(new_hash)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    sqlx::query("INSERT INTO local_recovery_codes(user_id,code_hash) VALUES($1,$2) ON CONFLICT(user_id) DO UPDATE SET code_hash=EXCLUDED.code_hash,created_at=NOW()")
        .bind(user_id).bind(message_hash(&recovery_code)).execute(&mut *tx).await.map_err(db_error)?;
    local_user::revoke_user_sessions(&mut tx, user_id).await?;
    sqlx::query(
        "UPDATE local_email_reset_tokens SET used_at=NOW() WHERE user_id=$1 AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    sqlx::query(
        "INSERT INTO email_events(actor_user_id,event_type) VALUES($1,'email_password_reset')",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(serde_json::json!({"recovery_code":recovery_code})))
}
