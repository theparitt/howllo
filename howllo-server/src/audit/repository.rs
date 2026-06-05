use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use crate::db::DbPool;

pub struct NewAuditLog {
    pub tenant_id: Uuid,
    pub actor_user_id: Uuid,
    pub entity_type: &'static str,
    pub entity_id: Uuid,
    pub action: &'static str,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub reason: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct AuditLogRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_user_id: Uuid,
    pub actor_display_name: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub action: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub reason: Option<String>,
    pub request_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert_audit_log(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry: NewAuditLog,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
            tenant_id,
            actor_user_id,
            entity_type,
            entity_id,
            action,
            old_value,
            new_value,
            reason,
            request_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(entry.tenant_id)
    .bind(entry.actor_user_id)
    .bind(entry.entity_type)
    .bind(entry.entity_id)
    .bind(entry.action)
    .bind(entry.old_value)
    .bind(entry.new_value)
    .bind(entry.reason)
    .bind(entry.request_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn insert_audit_log_on_pool(
    pool: &crate::db::DbPool,
    entry: NewAuditLog,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
            tenant_id,
            actor_user_id,
            entity_type,
            entity_id,
            action,
            old_value,
            new_value,
            reason,
            request_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(entry.tenant_id)
    .bind(entry.actor_user_id)
    .bind(entry.entity_type)
    .bind(entry.entity_id)
    .bind(entry.action)
    .bind(entry.old_value)
    .bind(entry.new_value)
    .bind(entry.reason)
    .bind(entry.request_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_audit_logs(pool: &DbPool, tenant_id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_logs WHERE tenant_id = $1")
        .bind(tenant_id)
        .fetch_one(pool)
        .await
}

pub async fn list_audit_logs(
    pool: &DbPool,
    tenant_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<AuditLogRow>, sqlx::Error> {
    sqlx::query_as::<_, AuditLogRow>(
        r#"
        SELECT
            a.id,
            a.tenant_id,
            a.actor_user_id,
            u.display_name AS actor_display_name,
            a.entity_type,
            a.entity_id,
            a.action,
            a.old_value,
            a.new_value,
            a.reason,
            a.request_id,
            a.created_at
        FROM audit_logs a
        JOIN users u ON u.id = a.actor_user_id
        WHERE a.tenant_id = $1
        ORDER BY a.created_at DESC, a.id DESC
        LIMIT $2
        OFFSET $3
        "#,
    )
    .bind(tenant_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}
