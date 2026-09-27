//! Platform-owned email transport, policy, and delivery queue.
//! No feature talks to SMTP directly; use `enqueue` and its typed wrappers.

use std::str::FromStr;
use std::time::Duration;

use actix_web::{get, post, put, web, HttpResponse, Responder};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, Address, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{
    auth::{local_admin, AuthenticatedUser},
    db::DbPool,
    errors::AppError,
};

#[derive(Clone, sqlx::FromRow)]
struct PlatformRow {
    smtp_host: String,
    smtp_port: i32,
    smtp_tls_mode: String,
    smtp_username: String,
    smtp_password_ciphertext: Option<String>,
    from_email: String,
    from_name: String,
    reply_to: String,
    status: String,
    config_version: i64,
    tested_at: Option<chrono::DateTime<chrono::Utc>>,
    failure_count: i32,
    monthly_limit: i32,
    tenant_daily_limit: i32,
    tenant_monthly_limit: i32,
    max_broadcast_recipients: i32,
    broadcasts_per_day: i32,
    broadcasts_per_week: i32,
}

#[derive(Serialize)]
struct PlatformEmailDto {
    smtp_host: String,
    smtp_port: i32,
    smtp_tls_mode: String,
    smtp_username: String,
    smtp_password_configured: bool,
    from_email: String,
    from_name: String,
    reply_to: String,
    status: String,
    tested_at: Option<chrono::DateTime<chrono::Utc>>,
    failure_count: i32,
    monthly_limit: i32,
    tenant_daily_limit: i32,
    tenant_monthly_limit: i32,
    max_broadcast_recipients: i32,
    broadcasts_per_day: i32,
    broadcasts_per_week: i32,
    monthly_used: i64,
    config_key_available: bool,
}

impl From<PlatformRow> for PlatformEmailDto {
    fn from(row: PlatformRow) -> Self {
        Self {
            smtp_host: row.smtp_host,
            smtp_port: row.smtp_port,
            smtp_tls_mode: row.smtp_tls_mode,
            smtp_username: row.smtp_username,
            smtp_password_configured: row.smtp_password_ciphertext.is_some(),
            from_email: row.from_email,
            from_name: row.from_name,
            reply_to: row.reply_to,
            status: row.status,
            tested_at: row.tested_at,
            failure_count: row.failure_count,
            monthly_limit: row.monthly_limit,
            tenant_daily_limit: row.tenant_daily_limit,
            tenant_monthly_limit: row.tenant_monthly_limit,
            max_broadcast_recipients: row.max_broadcast_recipients,
            broadcasts_per_day: row.broadcasts_per_day,
            broadcasts_per_week: row.broadcasts_per_week,
            monthly_used: 0,
            config_key_available: encryption_key().is_ok(),
        }
    }
}

#[derive(Deserialize)]
pub struct UpdatePlatformEmail {
    smtp_host: String,
    smtp_port: i32,
    smtp_tls_mode: String,
    smtp_username: String,
    /// Omit to keep the current secret. Empty string clears it.
    smtp_password: Option<String>,
    from_email: String,
    from_name: String,
    reply_to: String,
    monthly_limit: i32,
    tenant_daily_limit: i32,
    tenant_monthly_limit: i32,
    max_broadcast_recipients: i32,
    broadcasts_per_day: i32,
    broadcasts_per_week: i32,
}

#[derive(Deserialize)]
pub struct TestEmailRequest {
    recipient: String,
}

#[derive(Deserialize)]
pub struct SetEmailEnabled {
    enabled: bool,
}

#[derive(Deserialize)]
pub struct SuppressEmailRequest {
    recipient: String,
    reason: String,
}

#[derive(Deserialize)]
pub struct SuppressEmailQuery {
    recipient: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct SuppressedEmail {
    recipient: String,
    reason: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/api/admin/platform/email/suppression")]
pub async fn list_suppression(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    admin_only(&auth)?;
    let rows = sqlx::query_as::<_, SuppressedEmail>("SELECT recipient,reason,created_at FROM email_suppression ORDER BY created_at DESC LIMIT 100")
        .fetch_all(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::Ok().json(rows))
}

#[post("/api/admin/platform/email/suppression")]
pub async fn add_suppression(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<SuppressEmailRequest>,
) -> Result<impl Responder, AppError> {
    admin_only(&auth)?;
    mailbox(&body.recipient, None)?;
    let reason = body.reason.trim();
    if reason.len() < 3 || reason.len() > 200 {
        return Err(AppError::Validation(
            "Add a short suppression reason.".into(),
        ));
    }
    let recipient = body.recipient.trim().to_ascii_lowercase();
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("INSERT INTO email_suppression(recipient,reason) VALUES($1,$2) ON CONFLICT(recipient) DO UPDATE SET reason=EXCLUDED.reason")
        .bind(&recipient).bind(reason).execute(&mut *tx).await.map_err(db_error)?;
    sqlx::query("UPDATE email_messages SET status='cancelled',body='' WHERE recipient=$1 AND status='queued'")
        .bind(&recipient).execute(&mut *tx).await.map_err(db_error)?;
    sqlx::query("INSERT INTO email_events(actor_user_id,event_type,detail) VALUES($1,'suppression_added',$2)")
        .bind(auth.0.id).bind(&recipient).execute(&mut *tx).await.map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(HttpResponse::NoContent().finish())
}

#[actix_web::delete("/api/admin/platform/email/suppression")]
pub async fn remove_suppression(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<SuppressEmailQuery>,
) -> Result<impl Responder, AppError> {
    admin_only(&auth)?;
    let recipient = query
        .recipient
        .as_deref()
        .ok_or_else(|| AppError::Validation("Recipient is required.".into()))?;
    mailbox(recipient, None)?;
    let recipient = recipient.trim().to_ascii_lowercase();
    sqlx::query("DELETE FROM email_suppression WHERE recipient=$1")
        .bind(&recipient)
        .execute(pool.get_ref())
        .await
        .map_err(db_error)?;
    sqlx::query("INSERT INTO email_events(actor_user_id,event_type,detail) VALUES($1,'suppression_removed',$2)")
        .bind(auth.0.id).bind(&recipient).execute(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::NoContent().finish())
}

#[get("/api/email/availability")]
pub async fn availability(pool: web::Data<DbPool>) -> Result<impl Responder, AppError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "enabled": platform(pool.get_ref()).await?.status == "enabled"
    })))
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(%error, "email storage operation failed");
    AppError::InternalServerError
}

fn admin_only(auth: &AuthenticatedUser) -> Result<(), AppError> {
    if local_admin::is_local_admin_user(&auth.0) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

fn encryption_key() -> Result<Aes256Gcm, AppError> {
    let raw = std::env::var("HOWLLO_EMAIL_CONFIG_KEY").map_err(|_| {
        AppError::Validation(
            "Set HOWLLO_EMAIL_CONFIG_KEY on the server before configuring email.".into(),
        )
    })?;
    let bytes = hex::decode(raw).map_err(|_| {
        AppError::Validation("HOWLLO_EMAIL_CONFIG_KEY must be 64 hex characters.".into())
    })?;
    if bytes.len() != 32 {
        return Err(AppError::Validation(
            "HOWLLO_EMAIL_CONFIG_KEY must be 64 hex characters.".into(),
        ));
    }
    Aes256Gcm::new_from_slice(&bytes).map_err(|_| AppError::InternalServerError)
}

fn encrypt(plaintext: &str) -> Result<String, AppError> {
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let bytes = encryption_key()?
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|_| AppError::InternalServerError)?;
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(nonce),
        URL_SAFE_NO_PAD.encode(bytes)
    ))
}

fn decrypt(ciphertext: &str) -> Result<String, AppError> {
    let (nonce, bytes) = ciphertext
        .split_once('.')
        .ok_or(AppError::InternalServerError)?;
    let nonce = URL_SAFE_NO_PAD
        .decode(nonce)
        .map_err(|_| AppError::InternalServerError)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(bytes)
        .map_err(|_| AppError::InternalServerError)?;
    if nonce.len() != 12 {
        return Err(AppError::InternalServerError);
    }
    let plain = encryption_key()?
        .decrypt(Nonce::from_slice(&nonce), bytes.as_ref())
        .map_err(|_| AppError::InternalServerError)?;
    String::from_utf8(plain).map_err(|_| AppError::InternalServerError)
}

async fn platform(pool: &PgPool) -> Result<PlatformRow, AppError> {
    sqlx::query_as::<_, PlatformRow>(
        "SELECT smtp_host,smtp_port,smtp_tls_mode,smtp_username,smtp_password_ciphertext,from_email,from_name,reply_to,status,config_version,tested_at,failure_count,monthly_limit,tenant_daily_limit,tenant_monthly_limit,max_broadcast_recipients,broadcasts_per_day,broadcasts_per_week FROM email_platform_settings WHERE id=TRUE"
    ).fetch_one(pool).await.map_err(db_error)
}

async fn platform_dto(pool: &PgPool) -> Result<PlatformEmailDto, AppError> {
    let mut dto = PlatformEmailDto::from(platform(pool).await?);
    dto.monthly_used = sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE kind<>'system' AND created_at>=date_trunc('month',NOW()) AND status IN ('queued','sending','sent')")
        .fetch_one(pool).await.map_err(db_error)?;
    Ok(dto)
}

fn mailbox(email: &str, name: Option<&str>) -> Result<Mailbox, AppError> {
    let address = Address::from_str(email.trim())
        .map_err(|_| AppError::Validation("Invalid email address.".into()))?;
    Ok(Mailbox::new(
        name.filter(|value| !value.trim().is_empty())
            .map(ToString::to_string),
        address,
    ))
}

fn validate_config(input: &UpdatePlatformEmail) -> Result<(), AppError> {
    encryption_key()?;
    let host = input.smtp_host.trim();
    if host.is_empty()
        || host.len() > 253
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
        || !(1..=65535).contains(&input.smtp_port)
    {
        return Err(AppError::Validation(
            "Enter a valid SMTP host and port.".into(),
        ));
    }
    if !["implicit", "starttls", "none"].contains(&input.smtp_tls_mode.as_str()) {
        return Err(AppError::Validation(
            "TLS must be implicit, starttls, or none.".into(),
        ));
    }
    if input.smtp_tls_mode == "none" && !["localhost", "127.0.0.1", "::1"].contains(&host) {
        return Err(AppError::Validation(
            "Unencrypted SMTP is allowed only on loopback for local testing.".into(),
        ));
    }
    if input.smtp_tls_mode == "none" && !input.smtp_username.trim().is_empty() {
        return Err(AppError::Validation("SMTP credentials require TLS.".into()));
    }
    mailbox(&input.from_email, Some(&input.from_name))?;
    if !input.reply_to.trim().is_empty() {
        mailbox(&input.reply_to, None)?;
    }
    if input.from_name.chars().count() > 100
        || input.from_name.contains(['\r', '\n'])
        || input.smtp_username.len() > 255
        || input.smtp_username.contains(['\r', '\n'])
    {
        return Err(AppError::Validation(
            "Invalid sender or SMTP username.".into(),
        ));
    }
    for (value, max) in [
        (input.monthly_limit, 10_000_000),
        (input.tenant_daily_limit, 100_000),
        (input.tenant_monthly_limit, 1_000_000),
        (input.max_broadcast_recipients, 100_000),
    ] {
        if !(1..=max).contains(&value) {
            return Err(AppError::Validation("Email limit is out of range.".into()));
        }
    }
    if !(0..=100).contains(&input.broadcasts_per_day) {
        return Err(AppError::Validation(
            "Broadcast limit is out of range.".into(),
        ));
    }
    if !(0..=500).contains(&input.broadcasts_per_week) {
        return Err(AppError::Validation(
            "Weekly broadcast limit is out of range.".into(),
        ));
    }
    Ok(())
}

#[get("/api/admin/platform/email")]
pub async fn get_platform_email(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    admin_only(&auth)?;
    Ok(HttpResponse::Ok().json(platform_dto(pool.get_ref()).await?))
}

#[put("/api/admin/platform/email")]
pub async fn put_platform_email(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<UpdatePlatformEmail>,
) -> Result<impl Responder, AppError> {
    admin_only(&auth)?;
    validate_config(&body)?;
    let existing = platform(pool.get_ref()).await?;
    let password_ciphertext = match body.smtp_password.as_deref() {
        Some("") => None,
        Some(secret) => Some(encrypt(secret)?),
        None => existing.smtp_password_ciphertext.clone(),
    };
    let changed = existing.smtp_host != body.smtp_host.trim()
        || existing.smtp_port != body.smtp_port
        || existing.smtp_tls_mode != body.smtp_tls_mode
        || existing.smtp_username != body.smtp_username.trim()
        || existing.smtp_password_ciphertext != password_ciphertext
        || existing.from_email != body.from_email.trim()
        || existing.from_name != body.from_name.trim()
        || existing.reply_to != body.reply_to.trim();
    // Explicitly supplying the same password also changes the encrypted value and requires retest.
    sqlx::query("UPDATE email_platform_settings SET smtp_host=$1,smtp_port=$2,smtp_tls_mode=$3,smtp_username=$4,smtp_password_ciphertext=$5,from_email=$6,from_name=$7,reply_to=$8,monthly_limit=$9,tenant_daily_limit=$10,tenant_monthly_limit=$11,max_broadcast_recipients=$12,broadcasts_per_day=$13,broadcasts_per_week=$15,status=CASE WHEN $14 THEN 'configured' ELSE status END,tested_at=CASE WHEN $14 THEN NULL ELSE tested_at END,tested_version=CASE WHEN $14 THEN NULL ELSE tested_version END,config_version=CASE WHEN $14 THEN config_version+1 ELSE config_version END,failure_count=CASE WHEN $14 THEN 0 ELSE failure_count END,updated_at=NOW() WHERE id=TRUE")
        .bind(body.smtp_host.trim()).bind(body.smtp_port).bind(&body.smtp_tls_mode)
        .bind(body.smtp_username.trim()).bind(password_ciphertext).bind(body.from_email.trim())
        .bind(body.from_name.trim()).bind(body.reply_to.trim()).bind(body.monthly_limit)
        .bind(body.tenant_daily_limit).bind(body.tenant_monthly_limit).bind(body.max_broadcast_recipients)
        .bind(body.broadcasts_per_day).bind(changed).bind(body.broadcasts_per_week).execute(pool.get_ref()).await.map_err(db_error)?;
    sqlx::query(
        "INSERT INTO email_events(actor_user_id,event_type) VALUES($1,'platform_settings_changed')",
    )
    .bind(auth.0.id)
    .execute(pool.get_ref())
    .await
    .map_err(db_error)?;
    Ok(HttpResponse::Ok().json(platform_dto(pool.get_ref()).await?))
}

#[async_trait]
trait EmailProvider: Send + Sync {
    async fn send(&self, recipient: &str, subject: &str, body: &str) -> Result<(), AppError>;
}

struct SmtpProvider {
    config: PlatformRow,
}

#[async_trait]
impl EmailProvider for SmtpProvider {
    async fn send(&self, recipient: &str, subject: &str, body: &str) -> Result<(), AppError> {
        if self.config.smtp_tls_mode == "none"
            && !["localhost", "127.0.0.1", "::1"].contains(&self.config.smtp_host.as_str())
        {
            return Err(AppError::Validation(
                "Unencrypted SMTP requires loopback.".into(),
            ));
        }
        let from = mailbox(&self.config.from_email, Some(&self.config.from_name))?;
        let to = mailbox(recipient, None)?;
        let mut message = Message::builder().from(from).to(to).subject(subject);
        if !self.config.reply_to.is_empty() {
            message = message.reply_to(mailbox(&self.config.reply_to, None)?);
        }
        let message = message
            .body(body.to_string())
            .map_err(|_| AppError::Validation("Invalid email message.".into()))?;
        let mut builder = match self.config.smtp_tls_mode.as_str() {
            "implicit" => AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.smtp_host),
            "starttls" => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.smtp_host)
            }
            "none" => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
                &self.config.smtp_host,
            )),
            _ => return Err(AppError::InternalServerError),
        }
        .map_err(|_| AppError::Validation("Invalid SMTP relay.".into()))?
        .port(self.config.smtp_port as u16)
        .timeout(Some(Duration::from_secs(10)));
        if !self.config.smtp_username.is_empty() {
            let secret = self
                .config
                .smtp_password_ciphertext
                .as_deref()
                .ok_or(AppError::Validation("SMTP password is required.".into()))?;
            builder = builder.credentials(Credentials::new(
                self.config.smtp_username.clone(),
                decrypt(secret)?,
            ));
        }
        builder.build().send(message).await.map_err(|error| {
            tracing::warn!(error = %error, "SMTP delivery failed");
            AppError::BadRequest(
                "SMTP delivery failed. Check host, TLS mode, credentials, and recipient.".into(),
            )
        })?;
        Ok(())
    }
}

#[post("/api/admin/platform/email/test")]
pub async fn test_platform_email(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<TestEmailRequest>,
) -> Result<impl Responder, AppError> {
    admin_only(&auth)?;
    encryption_key()?;
    mailbox(&body.recipient, None)?;
    let row = platform(pool.get_ref()).await?;
    if row.smtp_host.is_empty() || row.from_email.is_empty() {
        return Err(AppError::Validation("Save SMTP settings first.".into()));
    }
    let attempts: i32 = sqlx::query_scalar("INSERT INTO local_auth_rate_limits(scope,identity_hash,window_start,attempts) VALUES('email-smtp-test',$1,NOW(),1) ON CONFLICT(scope,identity_hash) DO UPDATE SET attempts=CASE WHEN local_auth_rate_limits.window_start<NOW()-INTERVAL '1 hour' THEN 1 ELSE local_auth_rate_limits.attempts+1 END,window_start=CASE WHEN local_auth_rate_limits.window_start<NOW()-INTERVAL '1 hour' THEN NOW() ELSE local_auth_rate_limits.window_start END RETURNING attempts")
        .bind(message_hash(&body.recipient.trim().to_ascii_lowercase())).fetch_one(pool.get_ref()).await.map_err(db_error)?;
    if attempts > 5 {
        return Err(AppError::TooManyRequests(
            "Too many test emails to this address. Try again later.".into(),
        ));
    }
    let global_attempts: i32 = sqlx::query_scalar("INSERT INTO local_auth_rate_limits(scope,identity_hash,window_start,attempts) VALUES('email-smtp-test-global','all',NOW(),1) ON CONFLICT(scope,identity_hash) DO UPDATE SET attempts=CASE WHEN local_auth_rate_limits.window_start<NOW()-INTERVAL '1 hour' THEN 1 ELSE local_auth_rate_limits.attempts+1 END,window_start=CASE WHEN local_auth_rate_limits.window_start<NOW()-INTERVAL '1 hour' THEN NOW() ELSE local_auth_rate_limits.window_start END RETURNING attempts")
        .fetch_one(pool.get_ref()).await.map_err(db_error)?;
    if global_attempts > 20 {
        return Err(AppError::TooManyRequests(
            "Hourly SMTP test limit reached.".into(),
        ));
    }
    SmtpProvider {
        config: row.clone(),
    }
    .send(
        &body.recipient,
        "Howllo email test",
        "Your Howllo email connection is working.",
    )
    .await?;
    let updated = sqlx::query("UPDATE email_platform_settings SET status=CASE WHEN status='enabled' THEN 'enabled' ELSE 'tested' END,tested_at=NOW(),tested_version=config_version,failure_count=0 WHERE id=TRUE AND config_version=$1")
        .bind(row.config_version).execute(pool.get_ref()).await.map_err(db_error)?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Validation(
            "Email settings changed during the test. Test again.".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO email_events(actor_user_id,event_type) VALUES($1,'smtp_test_succeeded')",
    )
    .bind(auth.0.id)
    .execute(pool.get_ref())
    .await
    .map_err(db_error)?;
    Ok(HttpResponse::Ok().json(platform_dto(pool.get_ref()).await?))
}

#[post("/api/admin/platform/email/enabled")]
pub async fn set_platform_email_enabled(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<SetEmailEnabled>,
) -> Result<impl Responder, AppError> {
    admin_only(&auth)?;
    if body.enabled {
        encryption_key()?;
        let updated = sqlx::query("UPDATE email_platform_settings SET status='enabled',failure_count=0,updated_at=NOW() WHERE id=TRUE AND tested_version=config_version AND tested_at IS NOT NULL")
            .execute(pool.get_ref()).await.map_err(db_error)?;
        if updated.rows_affected() != 1 {
            return Err(AppError::Validation(
                "Send a successful test email before enabling delivery.".into(),
            ));
        }
    } else {
        sqlx::query(
            "UPDATE email_platform_settings SET status='disabled',updated_at=NOW() WHERE id=TRUE",
        )
        .execute(pool.get_ref())
        .await
        .map_err(db_error)?;
        sqlx::query("UPDATE email_messages SET status='cancelled',body='' WHERE status='queued'")
            .execute(pool.get_ref())
            .await
            .map_err(db_error)?;
    }
    sqlx::query("INSERT INTO email_events(actor_user_id,event_type) VALUES($1,$2)")
        .bind(auth.0.id)
        .bind(if body.enabled {
            "platform_enabled"
        } else {
            "platform_disabled"
        })
        .execute(pool.get_ref())
        .await
        .map_err(db_error)?;
    Ok(HttpResponse::Ok().json(platform_dto(pool.get_ref()).await?))
}

pub fn message_hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub mod account;
pub mod delivery;
pub mod workspace;

#[cfg(test)]
mod tests;
