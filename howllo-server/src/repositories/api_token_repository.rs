use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use crate::db::DbPool;

#[derive(Debug, FromRow)]
pub struct ApiTokenRecord {
    pub tenant_id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub scopes: Vec<String>,
    pub revoked_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, serde::Serialize, FromRow)]
pub struct ApiTokenListItemRow {
    pub id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub scopes: Vec<String>,
    pub revoked_at: Option<chrono::DateTime<Utc>>,
    pub created_at: chrono::DateTime<Utc>,
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn generate_token() -> (String, String, String) {
    let raw = Uuid::new_v4().simple().to_string();
    let prefix = raw.chars().take(8).collect::<String>();
    let token = format!("howllo_{prefix}_{raw}");
    (token, prefix, raw)
}

pub async fn list_api_tokens(
    pool: &DbPool,
    tenant_id: Uuid,
) -> Result<Vec<ApiTokenListItemRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiTokenListItemRow>(
        "SELECT id, name, token_prefix, scopes, revoked_at, created_at FROM api_tokens WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

pub async fn create_api_token(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    name: &str,
    token_prefix: &str,
    token_hash: &str,
    created_by: Uuid,
    scopes: &[String],
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO api_tokens (tenant_id, name, token_prefix, token_hash, created_by, scopes) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(tenant_id)
    .bind(name)
    .bind(token_prefix)
    .bind(token_hash)
    .bind(created_by)
    .bind(scopes)
    .fetch_one(&mut **tx)
    .await
}

pub async fn get_api_token(
    pool: &DbPool,
    token_id: Uuid,
) -> Result<Option<ApiTokenRecord>, sqlx::Error> {
    sqlx::query_as::<_, ApiTokenRecord>(
        "SELECT tenant_id, name, token_prefix, scopes, revoked_at FROM api_tokens WHERE id = $1",
    )
    .bind(token_id)
    .fetch_optional(pool)
    .await
}

#[derive(Debug, sqlx::FromRow)]
pub struct ApiTokenLookupRecord {
    pub tenant_id: Uuid,
    pub scopes: Vec<String>,
    pub revoked_at: Option<chrono::DateTime<Utc>>,
    pub expires_at: Option<chrono::DateTime<Utc>>,
}

pub async fn find_api_token_by_hash(
    pool: &DbPool,
    token_hash: &str,
) -> Result<Option<ApiTokenLookupRecord>, sqlx::Error> {
    sqlx::query_as::<_, ApiTokenLookupRecord>(
        "SELECT tenant_id, scopes, revoked_at, expires_at FROM api_tokens WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
}

pub async fn update_last_used_at(pool: &DbPool, token_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE api_tokens SET last_used_at = NOW() WHERE token_hash = $1",
        token_hash
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn revoke_api_token(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE api_tokens SET revoked_at = NOW() WHERE id = $1",
        token_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
