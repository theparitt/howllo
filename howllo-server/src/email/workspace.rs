use actix_web::{get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    auth::{require_permission, AuthenticatedUser},
    db::DbPool,
    domain::permission::Permission,
    errors::AppError,
    memberships,
    repositories::membership_repository,
};

use super::{
    db_error,
    delivery::{enqueue, EmailRequest, QueueResult},
    platform,
};

#[derive(Deserialize)]
pub struct TenantQuery {
    tenant_slug: String,
}

#[derive(Deserialize)]
pub struct UpdateTenantEmail {
    enabled: bool,
    reply_notifications: bool,
    important_updates: bool,
    digest_enabled: bool,
    broadcast_enabled: bool,
    daily_limit: Option<i32>,
    monthly_limit: Option<i32>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TenantEmailDto {
    enabled: bool,
    reply_notifications: bool,
    important_updates: bool,
    digest_enabled: bool,
    broadcast_enabled: bool,
    daily_limit: Option<i32>,
    monthly_limit: Option<i32>,
    effective_daily_limit: i32,
    effective_monthly_limit: i32,
    used_today: i64,
    used_month: i64,
}

async fn tenant_email(pool: &DbPool, tenant_id: Uuid) -> Result<TenantEmailDto, AppError> {
    let caps = platform(pool).await?;
    let row: Option<(bool,bool,bool,bool,bool,Option<i32>,Option<i32>)> = sqlx::query_as(
        "SELECT enabled,reply_notifications,important_updates,digest_enabled,broadcast_enabled,daily_limit,monthly_limit FROM tenant_email_settings WHERE tenant_id=$1"
    ).bind(tenant_id).fetch_optional(pool).await.map_err(db_error)?;
    let (enabled, replies, updates, digest, broadcast, daily, monthly) =
        row.unwrap_or((false, true, true, false, false, None, None));
    let used_today: i64 = sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE tenant_id=$1 AND kind<>'system' AND created_at>=date_trunc('day',NOW()) AND status IN ('queued','sending','sent')")
        .bind(tenant_id).fetch_one(pool).await.map_err(db_error)?;
    let used_month: i64 = sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE tenant_id=$1 AND kind<>'system' AND created_at>=date_trunc('month',NOW()) AND status IN ('queued','sending','sent')")
        .bind(tenant_id).fetch_one(pool).await.map_err(db_error)?;
    Ok(TenantEmailDto {
        enabled,
        reply_notifications: replies,
        important_updates: updates,
        digest_enabled: digest,
        broadcast_enabled: broadcast,
        daily_limit: daily,
        monthly_limit: monthly,
        effective_daily_limit: daily
            .unwrap_or(caps.tenant_daily_limit)
            .min(caps.tenant_daily_limit),
        effective_monthly_limit: monthly
            .unwrap_or(caps.tenant_monthly_limit)
            .min(caps.tenant_monthly_limit),
        used_today,
        used_month,
    })
}

async fn admin_scope(pool: &DbPool, slug: &str, user_id: Uuid) -> Result<Uuid, AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, slug).await?;
    require_permission(pool, tenant_id, user_id, Permission::ManageSettings).await?;
    if platform(pool).await?.status != "enabled" {
        return Err(AppError::NotFound);
    }
    Ok(tenant_id)
}

#[get("/api/admin/tenant-email")]
pub async fn get_tenant_email(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<TenantQuery>,
) -> Result<impl Responder, AppError> {
    let tenant_id = admin_scope(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(tenant_email(pool.get_ref(), tenant_id).await?))
}

#[put("/api/admin/tenant-email")]
pub async fn put_tenant_email(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<TenantQuery>,
    body: web::Json<UpdateTenantEmail>,
) -> Result<impl Responder, AppError> {
    let tenant_id = admin_scope(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    let caps = platform(pool.get_ref()).await?;
    if body
        .daily_limit
        .is_some_and(|n| n < 1 || n > caps.tenant_daily_limit)
        || body
            .monthly_limit
            .is_some_and(|n| n < 1 || n > caps.tenant_monthly_limit)
    {
        return Err(AppError::Validation(
            "Workspace email limits cannot exceed platform ceilings.".into(),
        ));
    }
    sqlx::query("INSERT INTO tenant_email_settings(tenant_id,enabled,reply_notifications,important_updates,digest_enabled,broadcast_enabled,daily_limit,monthly_limit) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(tenant_id) DO UPDATE SET enabled=EXCLUDED.enabled,reply_notifications=EXCLUDED.reply_notifications,important_updates=EXCLUDED.important_updates,digest_enabled=EXCLUDED.digest_enabled,broadcast_enabled=EXCLUDED.broadcast_enabled,daily_limit=EXCLUDED.daily_limit,monthly_limit=EXCLUDED.monthly_limit,updated_at=NOW()")
        .bind(tenant_id).bind(body.enabled).bind(body.reply_notifications).bind(body.important_updates)
        .bind(body.digest_enabled).bind(body.broadcast_enabled).bind(body.daily_limit).bind(body.monthly_limit)
        .execute(pool.get_ref()).await.map_err(db_error)?;
    if !body.enabled {
        sqlx::query("UPDATE email_messages SET status='cancelled',body='' WHERE tenant_id=$1 AND kind<>'system' AND status='queued'")
            .bind(tenant_id).execute(pool.get_ref()).await.map_err(db_error)?;
    }
    sqlx::query("INSERT INTO email_events(tenant_id,actor_user_id,event_type) VALUES($1,$2,'tenant_settings_changed')")
        .bind(tenant_id).bind(auth.0.id).execute(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::Ok().json(tenant_email(pool.get_ref(), tenant_id).await?))
}

#[derive(Serialize)]
pub struct UserEmailPrefsDto {
    verified_email: Option<String>,
    replies_enabled: bool,
    updates_enabled: bool,
    digest_enabled: bool,
    broadcast_enabled: bool,
}

#[derive(Deserialize)]
pub struct UpdateUserEmailPrefs {
    replies_enabled: bool,
    updates_enabled: bool,
    digest_enabled: bool,
    broadcast_enabled: bool,
}

async fn user_scope(pool: &DbPool, slug: &str, user_id: Uuid) -> Result<Uuid, AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, slug).await?;
    memberships::check_membership(pool, tenant_id, user_id)
        .await
        .map_err(|_| AppError::Forbidden)?;
    let p = platform(pool).await?;
    if p.status != "enabled" {
        return Err(AppError::NotFound);
    }
    let enabled: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT enabled FROM tenant_email_settings WHERE tenant_id=$1),FALSE)",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    if !enabled {
        return Err(AppError::NotFound);
    }
    Ok(tenant_id)
}

async fn prefs(
    pool: &DbPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<UserEmailPrefsDto, AppError> {
    let email: Option<String> =
        sqlx::query_scalar("SELECT email FROM user_verified_emails WHERE user_id=$1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?;
    let row: Option<(bool,bool,bool,bool)> = sqlx::query_as("SELECT replies_enabled,updates_enabled,digest_enabled,broadcast_enabled FROM user_email_preferences WHERE tenant_id=$1 AND user_id=$2")
        .bind(tenant_id).bind(user_id).fetch_optional(pool).await.map_err(db_error)?;
    let (replies, updates, digest, broadcast) = row.unwrap_or((true, true, false, false));
    Ok(UserEmailPrefsDto {
        verified_email: email,
        replies_enabled: replies,
        updates_enabled: updates,
        digest_enabled: digest,
        broadcast_enabled: broadcast,
    })
}

#[get("/api/me/email-preferences")]
pub async fn get_user_email_prefs(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<TenantQuery>,
) -> Result<impl Responder, AppError> {
    let tenant_id = user_scope(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(prefs(pool.get_ref(), tenant_id, auth.0.id).await?))
}

#[put("/api/me/email-preferences")]
pub async fn put_user_email_prefs(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<TenantQuery>,
    body: web::Json<UpdateUserEmailPrefs>,
) -> Result<impl Responder, AppError> {
    let tenant_id = user_scope(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    sqlx::query("INSERT INTO user_email_preferences(tenant_id,user_id,replies_enabled,updates_enabled,digest_enabled,broadcast_enabled) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(tenant_id,user_id) DO UPDATE SET replies_enabled=EXCLUDED.replies_enabled,updates_enabled=EXCLUDED.updates_enabled,digest_enabled=EXCLUDED.digest_enabled,broadcast_enabled=EXCLUDED.broadcast_enabled")
        .bind(tenant_id).bind(auth.0.id).bind(body.replies_enabled).bind(body.updates_enabled)
        .bind(body.digest_enabled).bind(body.broadcast_enabled).execute(pool.get_ref()).await.map_err(db_error)?;
    sqlx::query("UPDATE email_messages SET status='cancelled',body='' WHERE tenant_id=$1 AND user_id=$2 AND status='queued' AND ((category='reply' AND NOT $3) OR (category='update' AND NOT $4) OR (kind='digest' AND NOT $5) OR (kind='broadcast' AND NOT $6))")
        .bind(tenant_id).bind(auth.0.id).bind(body.replies_enabled).bind(body.updates_enabled)
        .bind(body.digest_enabled).bind(body.broadcast_enabled).execute(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::Ok().json(prefs(pool.get_ref(), tenant_id, auth.0.id).await?))
}

#[derive(Deserialize)]
pub struct BroadcastRequest {
    subject: String,
    body: String,
}

#[post("/api/admin/tenant-email/broadcast")]
pub async fn send_broadcast(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<TenantQuery>,
    body: web::Json<BroadcastRequest>,
) -> Result<impl Responder, AppError> {
    let tenant_id = admin_scope(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    let settings = tenant_email(pool.get_ref(), tenant_id).await?;
    if !settings.enabled || !settings.broadcast_enabled {
        return Err(AppError::Forbidden);
    }
    let subject = body.subject.trim();
    let content = body.body.trim();
    if subject.is_empty()
        || subject.len() > 180
        || subject.contains(['\r', '\n'])
        || content.is_empty()
        || content.len() > 10_000
    {
        return Err(AppError::Validation(
            "Announcement subject or body is invalid.".into(),
        ));
    }
    let caps = platform(pool.get_ref()).await?;
    let recipients = sqlx::query("SELECT e.user_id,e.email FROM user_verified_emails e JOIN memberships m ON m.user_id=e.user_id AND m.tenant_id=$1 AND m.public_participant=TRUE JOIN user_email_preferences p ON p.tenant_id=$1 AND p.user_id=e.user_id AND p.broadcast_enabled=TRUE LEFT JOIN email_suppression s ON s.recipient=e.email WHERE s.recipient IS NULL AND NOT EXISTS (SELECT 1 FROM workspace_restrictions r WHERE r.tenant_id=m.tenant_id AND r.user_id=m.user_id AND r.kind IN ('banned','suspended') AND (r.expires_at IS NULL OR r.expires_at>NOW())) ORDER BY e.user_id LIMIT $2")
        .bind(tenant_id).bind(i64::from(caps.max_broadcast_recipients)+1)
        .fetch_all(pool.get_ref()).await.map_err(db_error)?;
    if recipients.is_empty() {
        return Err(AppError::Validation(
            "No recipients have opted in to announcements.".into(),
        ));
    }
    if recipients.len() > caps.max_broadcast_recipients as usize {
        return Err(AppError::Validation(
            "Announcement exceeds the platform recipient limit.".into(),
        ));
    }
    let eligible = recipients.len();
    // Reserve the campaign under a database lock so two staff members cannot
    // both pass the daily count at the same time.
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended('howllo-broadcast:' || $1::text,0))",
    )
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    let sent_today: i64 = sqlx::query_scalar("SELECT count(*) FROM email_events WHERE tenant_id=$1 AND event_type='broadcast_created' AND created_at>=date_trunc('day',NOW())")
        .bind(tenant_id).fetch_one(&mut *tx).await.map_err(db_error)?;
    let sent_week: i64 = sqlx::query_scalar("SELECT count(*) FROM email_events WHERE tenant_id=$1 AND event_type='broadcast_created' AND created_at>=date_trunc('week',NOW())")
        .bind(tenant_id).fetch_one(&mut *tx).await.map_err(db_error)?;
    if sent_today >= i64::from(caps.broadcasts_per_day) {
        return Err(AppError::TooManyRequests(
            "Daily announcement limit reached.".into(),
        ));
    }
    if sent_week >= i64::from(caps.broadcasts_per_week) {
        return Err(AppError::TooManyRequests(
            "Weekly announcement limit reached.".into(),
        ));
    }
    sqlx::query("INSERT INTO email_events(tenant_id,actor_user_id,event_type,detail) VALUES($1,$2,'broadcast_created',$3)")
        .bind(tenant_id).bind(auth.0.id).bind(format!("eligible={eligible}"))
        .execute(&mut *tx).await.map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    let web_url = std::env::var("HOWLLO_CUSTOMER_WEB_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:7703".into());
    let message = format!("{content}\n\nTo stop these announcements, open {web_url}/{}/my/account and turn off Announcements in Email preferences.", query.tenant_slug);
    let campaign_id = Uuid::new_v4();
    let mut queued = 0;
    for recipient in recipients {
        let user_id: Uuid = recipient.get("user_id");
        let email: String = recipient.get("email");
        if matches!(
            enqueue(
                pool.get_ref(),
                EmailRequest {
                    tenant_id: Some(tenant_id),
                    user_id: Some(user_id),
                    recipient: &email,
                    kind: "broadcast",
                    category: "announcement",
                    subject,
                    body: &message,
                    dedupe_key: Some(format!("broadcast:{campaign_id}:{user_id}")),
                    delay_seconds: 0,
                }
            )
            .await?,
            QueueResult::Queued(_)
        ) {
            queued += 1;
        }
    }
    Ok(
        HttpResponse::Accepted()
            .json(serde_json::json!({ "queued": queued, "eligible": eligible })),
    )
}
