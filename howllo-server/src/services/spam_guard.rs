use chrono::{DateTime, Utc};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::errors::AppError;

pub async fn flag_account(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    reason: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO workspace_spam_flags (tenant_id, user_id, until_at, reason) VALUES ($1, $2, now() + interval '24 hours', $3) ON CONFLICT (tenant_id, user_id) DO UPDATE SET until_at = EXCLUDED.until_at, reason = EXCLUDED.reason",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(reason)
    .execute(&mut **tx)
    .await
    .map_err(|_| AppError::InternalServerError)?;
    Ok(())
}

pub async fn account_is_flagged(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM workspace_spam_flags WHERE tenant_id = $1 AND user_id = $2 AND until_at > now())")
        .bind(tenant_id)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| AppError::InternalServerError)
}

/// All writes to one board are serialized before checking its 10-minute volume.
/// A triggered cooldown is stored in Postgres, so it applies across server instances.
pub async fn board_is_paused(
    tx: &mut Transaction<'_, Postgres>,
    board_id: Uuid,
    kind: &str,
    limit: i32,
) -> Result<bool, AppError> {
    let lock_key = format!("board-write:{board_id}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(lock_key)
        .execute(&mut **tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    let until: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT until_at FROM board_write_cooldowns WHERE board_id = $1 AND kind = $2",
    )
    .bind(board_id)
    .bind(kind)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| AppError::InternalServerError)?;
    if until.is_some_and(|at| at > Utc::now()) {
        return Ok(true);
    }

    let count: i64 = if kind == "post" {
        sqlx::query("SELECT count(*)::bigint AS volume FROM posts WHERE board_id = $1 AND created_at > now() - interval '10 minutes'")
            .bind(board_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|_| AppError::InternalServerError)?
            .get("volume")
    } else {
        sqlx::query("SELECT count(*)::bigint AS volume FROM comments c JOIN posts p ON p.id = c.post_id WHERE p.board_id = $1 AND c.created_at > now() - interval '10 minutes'")
            .bind(board_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|_| AppError::InternalServerError)?
            .get("volume")
    };
    if count < i64::from(limit) {
        return Ok(false);
    }

    sqlx::query("INSERT INTO board_write_cooldowns (board_id, kind, until_at) VALUES ($1, $2, now() + interval '5 minutes') ON CONFLICT (board_id, kind) DO UPDATE SET until_at = EXCLUDED.until_at")
        .bind(board_id)
        .bind(kind)
        .execute(&mut **tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;
    Ok(true)
}
