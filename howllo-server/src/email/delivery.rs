use chrono::{Timelike, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::AppError;

use super::{
    db_error, decrypt, encrypt, mailbox, message_hash, platform, EmailProvider, SmtpProvider,
};

pub struct EmailRequest<'a> {
    pub tenant_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub recipient: &'a str,
    pub kind: &'a str,
    pub category: &'a str,
    pub subject: &'a str,
    pub body: &'a str,
    pub dedupe_key: Option<String>,
    pub delay_seconds: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum QueueResult {
    Queued(Uuid),
    Duplicate,
    Skipped(&'static str),
}

pub(super) fn prefix_utf8(value: &str, max_bytes: usize) -> String {
    let mut out = String::new();
    for character in value.chars() {
        if out.len() + character.len_utf8() > max_bytes {
            break;
        }
        out.push(character);
    }
    out
}

pub async fn enqueue(pool: &PgPool, request: EmailRequest<'_>) -> Result<QueueResult, AppError> {
    mailbox(request.recipient, None)?;
    if !["system", "notification", "digest", "broadcast"].contains(&request.kind)
        || request.subject.trim().is_empty()
        || request.subject.len() > 200
        || request.subject.contains(['\r', '\n'])
        || request.body.len() > 20_000
        || request.category.len() > 50
        || request.delay_seconds < 0
        || request.delay_seconds > 86_400
    {
        return Err(AppError::Validation("Invalid email request.".into()));
    }
    let recipient = request.recipient.trim().to_ascii_lowercase();
    let mut tx = pool.begin().await.map_err(db_error)?;
    // Serialize admissions across API instances. Both deduplication and quota checks
    // are made against committed queue rows while this transaction holds the lock.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('howllo-email-budget',0))")
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    let settings = platform(pool).await?;
    if settings.status != "enabled" {
        return Ok(QueueResult::Skipped("platform_disabled"));
    }
    if let Some(ref key) = request.dedupe_key {
        let found: Option<(Uuid, String)> =
            sqlx::query_as("SELECT id,status FROM email_messages WHERE dedupe_key=$1")
                .bind(key)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
        if let Some((id, status)) = found {
            if status == "queued" && request.kind == "notification" {
                sqlx::query(
                    "UPDATE email_messages SET event_count=LEAST(event_count+1,100) WHERE id=$1",
                )
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(db_error)?;
                tx.commit().await.map_err(db_error)?;
            }
            return Ok(QueueResult::Duplicate);
        }
    }
    let suppressed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM email_suppression WHERE recipient=$1)")
            .bind(&recipient)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_error)?;
    if suppressed {
        return Ok(QueueResult::Skipped("suppressed"));
    }
    if request.kind != "system" {
        let (Some(tenant_id), Some(user_id)) = (request.tenant_id, request.user_id) else {
            return Err(AppError::Validation(
                "Tenant email requires a recipient account.".into(),
            ));
        };
        let verified: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_verified_emails WHERE user_id=$1 AND email=$2)",
        )
        .bind(user_id)
        .bind(&recipient)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        if !verified {
            return Ok(QueueResult::Skipped("unverified_email"));
        }
        let member: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM memberships m WHERE m.tenant_id=$1 AND m.user_id=$2 AND ($3<>'broadcast' OR m.public_participant=TRUE) AND NOT EXISTS (SELECT 1 FROM workspace_restrictions r WHERE r.tenant_id=m.tenant_id AND r.user_id=m.user_id AND r.kind IN ('banned','suspended') AND (r.expires_at IS NULL OR r.expires_at>NOW())))")
            .bind(tenant_id).bind(user_id).bind(request.kind).fetch_one(&mut *tx).await.map_err(db_error)?;
        if !member {
            return Ok(QueueResult::Skipped("not_workspace_member"));
        }
        let enabled: Option<(bool,bool,bool,bool,bool,Option<i32>,Option<i32>)> = sqlx::query_as(
            "SELECT s.enabled,s.reply_notifications,s.important_updates,s.digest_enabled,s.broadcast_enabled,s.daily_limit,s.monthly_limit FROM tenant_email_settings s WHERE s.tenant_id=$1"
        ).bind(tenant_id).fetch_optional(&mut *tx).await.map_err(db_error)?;
        let Some((
            tenant_enabled,
            replies,
            updates,
            digest,
            broadcast,
            daily_override,
            monthly_override,
        )) = enabled
        else {
            return Ok(QueueResult::Skipped("tenant_disabled"));
        };
        if !tenant_enabled {
            return Ok(QueueResult::Skipped("tenant_disabled"));
        }
        let allowed = match request.category {
            "reply" => replies,
            "update" => updates,
            _ => match request.kind {
                "digest" => digest,
                "broadcast" => broadcast,
                _ => false,
            },
        };
        if !allowed {
            return Ok(QueueResult::Skipped("feature_disabled"));
        }
        let prefs: Option<(bool,bool,bool,bool)> = sqlx::query_as(
            "SELECT replies_enabled,updates_enabled,digest_enabled,broadcast_enabled FROM user_email_preferences WHERE tenant_id=$1 AND user_id=$2"
        ).bind(tenant_id).bind(user_id).fetch_optional(&mut *tx).await.map_err(db_error)?;
        let user_allowed = match request.category {
            "reply" => prefs.map_or(true, |p| p.0),
            "update" => prefs.map_or(true, |p| p.1),
            _ => match request.kind {
                "digest" => prefs.is_some_and(|p| p.2),
                "broadcast" => prefs.is_some_and(|p| p.3),
                _ => false,
            },
        };
        if !user_allowed {
            return Ok(QueueResult::Skipped("user_opt_out"));
        }
        let day_count: i64 = sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE tenant_id=$1 AND kind<>'system' AND created_at >= date_trunc('day',NOW()) AND status IN ('queued','sending','sent')")
            .bind(tenant_id).fetch_one(&mut *tx).await.map_err(db_error)?;
        let month_count: i64 = sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE tenant_id=$1 AND kind<>'system' AND created_at >= date_trunc('month',NOW()) AND status IN ('queued','sending','sent')")
            .bind(tenant_id).fetch_one(&mut *tx).await.map_err(db_error)?;
        let daily = daily_override
            .unwrap_or(settings.tenant_daily_limit)
            .min(settings.tenant_daily_limit);
        let monthly = monthly_override
            .unwrap_or(settings.tenant_monthly_limit)
            .min(settings.tenant_monthly_limit);
        if day_count >= i64::from(daily) || month_count >= i64::from(monthly) {
            return Ok(QueueResult::Skipped("tenant_quota"));
        }
    }
    let recipient_today: i64 = sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE recipient=$1 AND created_at >= date_trunc('day',NOW()) AND status IN ('queued','sending','sent')")
        .bind(&recipient).fetch_one(&mut *tx).await.map_err(db_error)?;
    let recipient_cap =
        if request.category == "password_reset" || request.category == "email_verification" {
            5
        } else {
            20
        };
    if recipient_today >= recipient_cap {
        return Ok(QueueResult::Skipped("recipient_rate_limit"));
    }
    if request.kind != "system" {
        let platform_month: i64 = sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE created_at >= date_trunc('month',NOW()) AND kind <> 'system' AND status IN ('queued','sending','sent')")
            .fetch_one(&mut *tx).await.map_err(db_error)?;
        if platform_month >= i64::from(settings.monthly_limit) {
            return Ok(QueueResult::Skipped("platform_budget"));
        }
    }
    let encrypted_body = encrypt(request.body)?;
    let id: Uuid = sqlx::query_scalar("INSERT INTO email_messages(tenant_id,user_id,recipient,kind,category,subject,body,dedupe_key,next_attempt_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW()+($9::int*INTERVAL '1 second')) RETURNING id")
        .bind(request.tenant_id).bind(request.user_id).bind(&recipient).bind(request.kind)
        .bind(request.category).bind(request.subject).bind(encrypted_body)
        .bind(request.dedupe_key).bind(request.delay_seconds)
        .fetch_one(&mut *tx).await.map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(QueueResult::Queued(id))
}

pub async fn enqueue_invitation(
    pool: &PgPool,
    tenant_id: Uuid,
    invitation_id: Uuid,
    email: &str,
    workspace_name: &str,
) -> Result<QueueResult, AppError> {
    let app_url =
        std::env::var("HOWLLO_STAFF_APP_URL").unwrap_or_else(|_| "http://localhost:7702".into());
    let body = format!("You have been invited to join {workspace_name} on Howllo.\n\nSign in to review the invitation: {app_url}\n\nIf you did not expect this, you can ignore this message.");
    enqueue(
        pool,
        EmailRequest {
            tenant_id: Some(tenant_id),
            user_id: None,
            recipient: email,
            kind: "system",
            category: "invitation",
            subject: "Your Howllo workspace invitation",
            body: &body,
            dedupe_key: Some(format!("invitation:{invitation_id}")),
            delay_seconds: 0,
        },
    )
    .await
}

pub async fn enqueue_notification(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    event_type: &str,
    title: &str,
    body: &str,
    post_id: Option<Uuid>,
) -> Result<QueueResult, AppError> {
    let category = match event_type {
        "post_comment" => "reply",
        "status_changed" | "official_response" | "post_approved" | "post_rejected" => "update",
        _ => return Ok(QueueResult::Skipped("unsupported_event")),
    };
    let recipient: Option<String> =
        sqlx::query_scalar("SELECT email FROM user_verified_emails WHERE user_id=$1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?;
    let Some(recipient) = recipient else {
        return Ok(QueueResult::Skipped("unverified_email"));
    };
    let bucket = Utc::now().timestamp() / 900;
    let key = message_hash(&format!(
        "{tenant_id}:{user_id}:{category}:{post_id:?}:{bucket}"
    ));
    let safe_title = prefix_utf8(title, 180);
    let safe_body = body.chars().take(2000).collect::<String>();
    let slug: String = sqlx::query_scalar("SELECT slug FROM tenants WHERE id=$1")
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    let web_url = std::env::var("HOWLLO_CUSTOMER_WEB_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:7703".into());
    let message_body = if let Some(post_id) = post_id {
        format!("{safe_body}\n\nRead the discussion: {web_url}/{slug}/posts/{post_id}\n\nChange email preferences in your account.")
    } else {
        format!("{safe_body}\n\nOpen your board: {web_url}/{slug}\n\nChange email preferences in your account.")
    };
    enqueue(
        pool,
        EmailRequest {
            tenant_id: Some(tenant_id),
            user_id: Some(user_id),
            recipient: &recipient,
            kind: "notification",
            category,
            subject: &safe_title,
            body: &message_body,
            dedupe_key: Some(key),
            delay_seconds: 300,
        },
    )
    .await
}

#[derive(sqlx::FromRow)]
struct ClaimedMessage {
    id: Uuid,
    tenant_id: Option<Uuid>,
    user_id: Option<Uuid>,
    recipient: String,
    kind: String,
    category: String,
    subject: String,
    body: String,
    attempts: i32,
    event_count: i32,
}

pub async fn process_one(pool: &PgPool) -> Result<bool, AppError> {
    let config = platform(pool).await?;
    if config.status != "enabled" {
        return Ok(false);
    }
    let row: Option<ClaimedMessage> = sqlx::query_as(
        "UPDATE email_messages SET status='sending',attempts=attempts+1,claimed_at=NOW() WHERE id=(SELECT id FROM email_messages WHERE (status='queued' AND next_attempt_at<=NOW()) OR (status='sending' AND claimed_at<NOW()-INTERVAL '2 minutes') ORDER BY CASE kind WHEN 'system' THEN 0 WHEN 'notification' THEN 1 WHEN 'digest' THEN 2 ELSE 3 END,next_attempt_at,created_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING id,tenant_id,user_id,recipient,kind,category,subject,body,attempts,event_count"
    ).fetch_optional(pool).await.map_err(db_error)?;
    let Some(message) = row else {
        return Ok(false);
    };
    if message.kind != "system" {
        // Recheck consent at send time: members may leave or opt out after a
        // notification has entered the queue.
        let enabled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_email_settings s JOIN memberships m ON m.tenant_id=s.tenant_id AND m.user_id=$2 JOIN user_verified_emails e ON e.user_id=$2 AND e.email=$3 LEFT JOIN user_email_preferences p ON p.tenant_id=s.tenant_id AND p.user_id=$2 WHERE s.tenant_id=$1 AND s.enabled=TRUE AND ($5 <> 'broadcast' OR m.public_participant=TRUE) AND NOT EXISTS (SELECT 1 FROM workspace_restrictions r WHERE r.tenant_id=s.tenant_id AND r.user_id=$2 AND r.kind IN ('banned','suspended') AND (r.expires_at IS NULL OR r.expires_at>NOW())) AND CASE WHEN $4='reply' THEN s.reply_notifications AND COALESCE(p.replies_enabled,TRUE) WHEN $4='update' THEN s.important_updates AND COALESCE(p.updates_enabled,TRUE) WHEN $5='digest' THEN s.digest_enabled AND COALESCE(p.digest_enabled,FALSE) WHEN $5='broadcast' THEN s.broadcast_enabled AND COALESCE(p.broadcast_enabled,FALSE) ELSE FALSE END)")
            .bind(message.tenant_id).bind(message.user_id).bind(&message.recipient)
            .bind(&message.category).bind(&message.kind).fetch_one(pool).await.map_err(db_error)?;
        if !enabled {
            sqlx::query("UPDATE email_messages SET status='cancelled',body='' WHERE id=$1")
                .bind(message.id)
                .execute(pool)
                .await
                .map_err(db_error)?;
            return Ok(true);
        }
    }
    let suppressed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM email_suppression WHERE recipient=$1)")
            .bind(&message.recipient)
            .fetch_one(pool)
            .await
            .map_err(db_error)?;
    if suppressed {
        sqlx::query("UPDATE email_messages SET status='cancelled',body='' WHERE id=$1")
            .bind(message.id)
            .execute(pool)
            .await
            .map_err(db_error)?;
        return Ok(true);
    }
    let body = match decrypt(&message.body) {
        Ok(body) => body,
        Err(error) => {
            sqlx::query("UPDATE email_messages SET status='queued',claimed_at=NULL,next_attempt_at=NOW()+INTERVAL '5 minutes',last_error='Message encryption key unavailable' WHERE id=$1")
                .bind(message.id).execute(pool).await.map_err(db_error)?;
            sqlx::query(
                "UPDATE email_platform_settings SET status='paused',failure_count=5 WHERE id=TRUE",
            )
            .execute(pool)
            .await
            .map_err(db_error)?;
            return Err(error);
        }
    };
    let body = if message.event_count > 1 && message.category == "reply" {
        format!(
            "{} new replies in this discussion.\n\n{}",
            message.event_count, body
        )
    } else {
        body
    };
    let result = SmtpProvider { config }
        .send(&message.recipient, &message.subject, &body)
        .await;
    match result {
        Ok(()) => {
            sqlx::query("UPDATE email_messages SET status='sent',sent_at=NOW(),body='',last_error=NULL WHERE id=$1")
                .bind(message.id).execute(pool).await.map_err(db_error)?;
            sqlx::query("UPDATE email_platform_settings SET failure_count=0 WHERE id=TRUE")
                .execute(pool)
                .await
                .map_err(db_error)?;
            sqlx::query(
                "INSERT INTO email_events(tenant_id,message_id,event_type) VALUES($1,$2,'sent')",
            )
            .bind(message.tenant_id)
            .bind(message.id)
            .execute(pool)
            .await
            .map_err(db_error)?;
        }
        Err(_) => {
            let retry_seconds =
                30_i32.saturating_mul(2_i32.pow(message.attempts.saturating_sub(1).min(6) as u32));
            sqlx::query("UPDATE email_messages SET status=CASE WHEN attempts>=5 THEN 'failed' ELSE 'queued' END,next_attempt_at=NOW()+($2::int*INTERVAL '1 second'),last_error='SMTP delivery failed' WHERE id=$1")
                .bind(message.id).bind(retry_seconds).execute(pool).await.map_err(db_error)?;
            sqlx::query("UPDATE email_platform_settings SET failure_count=failure_count+1,status=CASE WHEN failure_count+1>=5 THEN 'paused' ELSE status END WHERE id=TRUE AND status='enabled'")
                .execute(pool).await.map_err(db_error)?;
            sqlx::query("INSERT INTO email_events(tenant_id,message_id,event_type) VALUES($1,$2,'delivery_failed')")
                .bind(message.tenant_id).bind(message.id).execute(pool).await.map_err(db_error)?;
        }
    }
    Ok(true)
}

pub fn start_worker(pool: PgPool) {
    let digest_pool = pool.clone();
    tokio::spawn(async move {
        loop {
            if let Err(error) = process_one(&pool).await {
                tracing::error!(%error, "email worker tick failed");
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
    tokio::spawn(async move {
        loop {
            if chrono::Utc::now().hour() >= 8 {
                if let Err(error) = enqueue_daily_digests(&digest_pool).await {
                    tracing::error!(%error, "daily email digest scheduling failed");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });
}

pub async fn enqueue_daily_digests(pool: &PgPool) -> Result<usize, AppError> {
    if platform(pool).await?.status != "enabled" {
        return Ok(0);
    }
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let rows = sqlx::query("SELECT s.tenant_id,p.user_id,e.email,t.name,t.slug,count(n.id)::bigint AS unread FROM tenant_email_settings s JOIN tenants t ON t.id=s.tenant_id JOIN user_email_preferences p ON p.tenant_id=s.tenant_id AND p.digest_enabled=TRUE JOIN memberships m ON m.tenant_id=s.tenant_id AND m.user_id=p.user_id JOIN user_verified_emails e ON e.user_id=p.user_id JOIN notifications n ON n.tenant_id=s.tenant_id AND n.user_id=p.user_id AND n.is_read=FALSE AND n.created_at>=NOW()-INTERVAL '1 day' WHERE s.enabled=TRUE AND s.digest_enabled=TRUE AND NOT EXISTS (SELECT 1 FROM workspace_restrictions r WHERE r.tenant_id=s.tenant_id AND r.user_id=p.user_id AND r.kind IN ('banned','suspended') AND (r.expires_at IS NULL OR r.expires_at>NOW())) AND NOT EXISTS (SELECT 1 FROM email_messages em WHERE em.dedupe_key='digest:' || $1 || ':' || s.tenant_id::text || ':' || p.user_id::text) GROUP BY s.tenant_id,p.user_id,e.email,t.name,t.slug ORDER BY s.tenant_id,p.user_id LIMIT 5000")
        .bind(&day).fetch_all(pool).await.map_err(db_error)?;
    let mut queued = 0;
    for row in rows {
        let tenant_id: Uuid = row.get("tenant_id");
        let user_id: Uuid = row.get("user_id");
        let email: String = row.get("email");
        let name: String = row.get("name");
        let slug: String = row.get("slug");
        let unread: i64 = row.get("unread");
        let subject = format!("Your {} board digest", prefix_utf8(&name, 170));
        let web_url = std::env::var("HOWLLO_CUSTOMER_WEB_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:7703".into());
        let body = format!("You have {unread} unread updates in {name}.\n\nOpen {web_url}/{slug} to read them.\n\nYou can turn off daily digests in your account email preferences.");
        if matches!(
            enqueue(
                pool,
                EmailRequest {
                    tenant_id: Some(tenant_id),
                    user_id: Some(user_id),
                    recipient: &email,
                    kind: "digest",
                    category: "digest",
                    subject: &subject,
                    body: &body,
                    dedupe_key: Some(format!("digest:{day}:{tenant_id}:{user_id}")),
                    delay_seconds: 0
                }
            )
            .await?,
            QueueResult::Queued(_)
        ) {
            queued += 1;
        }
    }
    Ok(queued)
}
