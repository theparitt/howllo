use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::db::DbPool;

pub struct WebhookEndpointRecord {
    pub id: Uuid,
    pub url: String,
    pub secret: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

pub async fn list_webhooks(
    pool: &DbPool,
    tenant_id: Uuid,
) -> Result<Vec<WebhookEndpointRecord>, sqlx::Error> {
    sqlx::query_as!(
        WebhookEndpointRecord,
        "SELECT id, url, secret, is_active, created_at FROM webhook_endpoints WHERE tenant_id = $1 ORDER BY created_at DESC",
        tenant_id
    )
    .fetch_all(pool)
    .await
}

pub async fn create_webhook(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    url: &str,
    secret: Option<&str>,
) -> Result<WebhookEndpointRecord, sqlx::Error> {
    sqlx::query_as!(
        WebhookEndpointRecord,
        "INSERT INTO webhook_endpoints (tenant_id, url, secret) VALUES ($1, $2, $3) RETURNING id, url, secret, is_active, created_at",
        tenant_id,
        url,
        secret
    )
    .fetch_one(&mut **tx)
    .await
}

pub async fn get_webhook_tenant(
    pool: &DbPool,
    webhook_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT tenant_id FROM webhook_endpoints WHERE id = $1",
        webhook_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| row.tenant_id))
}

pub async fn deactivate_webhook(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    webhook_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE webhook_endpoints SET is_active = FALSE WHERE id = $1",
        webhook_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn create_webhook_event(
    pool: &DbPool,
    tenant_id: Uuid,
    event_type: &str,
    payload: &serde_json::Value,
) -> Result<Uuid, sqlx::Error> {
    let row = sqlx::query!(
        "INSERT INTO webhook_events (tenant_id, event_type, payload) VALUES ($1, $2, $3) RETURNING id",
        tenant_id,
        event_type,
        payload
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn get_active_endpoints(
    pool: &DbPool,
    tenant_id: Uuid,
) -> Result<Vec<(Uuid, String, Option<String>)>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT id, url, secret FROM webhook_endpoints WHERE tenant_id = $1 AND is_active = TRUE",
        tenant_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.id, row.url, row.secret))
        .collect())
}

pub async fn record_webhook_delivery(
    pool: &DbPool,
    event_id: Uuid,
    endpoint_id: Uuid,
    status_code: i32,
    success: bool,
    response_body: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO webhook_deliveries (webhook_event_id, webhook_endpoint_id, status_code, success, response_body) VALUES ($1, $2, $3, $4, $5)",
        event_id,
        endpoint_id,
        status_code,
        success,
        response_body
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn record_webhook_delivery_failure(
    pool: &DbPool,
    event_id: Uuid,
    endpoint_id: Uuid,
    error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO webhook_deliveries (webhook_event_id, webhook_endpoint_id, success, response_body) VALUES ($1, $2, FALSE, $3)",
        event_id,
        endpoint_id,
        error
    )
    .execute(pool)
    .await?;
    Ok(())
}
