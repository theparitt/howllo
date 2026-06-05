use serde_json::Value;
use uuid::Uuid;

use crate::audit::repository::{self, NewAuditLog};
use crate::errors::AppError;
use crate::http;

pub struct AuditEntry {
    pub tenant_id: Uuid,
    pub actor_user_id: Uuid,
    pub entity_type: &'static str,
    pub entity_id: Uuid,
    pub action: &'static str,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub reason: Option<String>,
}

pub async fn record_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry: AuditEntry,
) -> Result<(), AppError> {
    repository::insert_audit_log(
        tx,
        NewAuditLog {
            tenant_id: entry.tenant_id,
            actor_user_id: entry.actor_user_id,
            entity_type: entry.entity_type,
            entity_id: entry.entity_id,
            action: entry.action,
            old_value: entry.old_value,
            new_value: entry.new_value,
            reason: entry.reason,
            request_id: http::current_request_id(),
        },
    )
    .await
    .map_err(|error| {
        tracing::error!(
            error = %error,
            tenant_id = %entry.tenant_id,
            actor_user_id = %entry.actor_user_id,
            entity_type = entry.entity_type,
            entity_id = %entry.entity_id,
            action = entry.action,
            "error writing audit log"
        );
        AppError::InternalServerError
    })
}

pub async fn record(pool: &crate::db::DbPool, entry: AuditEntry) -> Result<(), AppError> {
    repository::insert_audit_log_on_pool(
        pool,
        NewAuditLog {
            tenant_id: entry.tenant_id,
            actor_user_id: entry.actor_user_id,
            entity_type: entry.entity_type,
            entity_id: entry.entity_id,
            action: entry.action,
            old_value: entry.old_value,
            new_value: entry.new_value,
            reason: entry.reason,
            request_id: http::current_request_id(),
        },
    )
    .await
    .map_err(|error| {
        tracing::error!(
            error = %error,
            tenant_id = %entry.tenant_id,
            actor_user_id = %entry.actor_user_id,
            entity_type = entry.entity_type,
            entity_id = %entry.entity_id,
            action = entry.action,
            "error writing audit log"
        );
        AppError::InternalServerError
    })
}
