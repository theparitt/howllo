use uuid::Uuid;

use crate::db::DbPool;

/// A raw invitation row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InvitationRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub role: String,
    pub status: String,
    pub invited_by: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub responded_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// An invitation joined with the workspace + inviter for display.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct InvitationListItem {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub status: String,
    pub tenant_slug: String,
    pub tenant_name: String,
    pub invited_by_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

const ROW_COLS: &str =
    "id, tenant_id, email, role, status, invited_by, user_id, created_at, responded_at";

/// Does a real account already exist for this (lowercased) email? Used to stamp
/// user_id at invite time so the notification/feed can reach them immediately.
pub async fn find_user_id_by_email(
    pool: &DbPool,
    email_lower: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM users WHERE lower(email) = $1 LIMIT 1")
        .bind(email_lower)
        .fetch_optional(pool)
        .await
}

pub async fn create(
    pool: &DbPool,
    tenant_id: Uuid,
    email_lower: &str,
    role: &str,
    invited_by: Uuid,
    user_id: Option<Uuid>,
) -> Result<InvitationRow, sqlx::Error> {
    sqlx::query_as::<_, InvitationRow>(&format!(
        r#"
        INSERT INTO workspace_invitations (tenant_id, email, role, invited_by, user_id)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING {ROW_COLS}
        "#
    ))
    .bind(tenant_id)
    .bind(email_lower)
    .bind(role)
    .bind(invited_by)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

pub async fn get(pool: &DbPool, id: Uuid) -> Result<Option<InvitationRow>, sqlx::Error> {
    sqlx::query_as::<_, InvitationRow>(&format!(
        "SELECT {ROW_COLS} FROM workspace_invitations WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_for_tenant(
    pool: &DbPool,
    tenant_id: Uuid,
) -> Result<Vec<InvitationListItem>, sqlx::Error> {
    sqlx::query_as::<_, InvitationListItem>(
        r#"
        SELECT i.id, i.email, i.role, i.status,
               t.slug AS tenant_slug, t.name AS tenant_name,
               u.display_name AS invited_by_name,
               i.created_at
        FROM workspace_invitations i
        JOIN tenants t ON t.id = i.tenant_id
        LEFT JOIN users u ON u.id = i.invited_by
        WHERE i.tenant_id = $1
        ORDER BY i.created_at DESC
        "#,
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

pub async fn list_pending_for_user(
    pool: &DbPool,
    user_id: Uuid,
) -> Result<Vec<InvitationListItem>, sqlx::Error> {
    sqlx::query_as::<_, InvitationListItem>(
        r#"
        SELECT i.id, i.email, i.role, i.status,
               t.slug AS tenant_slug, t.name AS tenant_name,
               u.display_name AS invited_by_name,
               i.created_at
        FROM workspace_invitations i
        JOIN tenants t ON t.id = i.tenant_id
        LEFT JOIN users u ON u.id = i.invited_by
        WHERE i.user_id = $1 AND i.status = 'pending'
        ORDER BY i.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Mark a pending invite for a tenant as withdrawn. Returns the row if it was
/// pending (so the caller can notify the invitee).
pub async fn withdraw(
    pool: &DbPool,
    id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<InvitationRow>, sqlx::Error> {
    sqlx::query_as::<_, InvitationRow>(&format!(
        r#"
        UPDATE workspace_invitations
        SET status = 'withdrawn', responded_at = NOW()
        WHERE id = $1 AND tenant_id = $2 AND status = 'pending'
        RETURNING {ROW_COLS}
        "#
    ))
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

/// Transition a pending invite owned by `user_id` to accepted/rejected.
pub async fn respond(
    pool: &DbPool,
    id: Uuid,
    user_id: Uuid,
    new_status: &str,
) -> Result<Option<InvitationRow>, sqlx::Error> {
    sqlx::query_as::<_, InvitationRow>(&format!(
        r#"
        UPDATE workspace_invitations
        SET status = $3, responded_at = NOW()
        WHERE id = $1 AND user_id = $2 AND status = 'pending'
        RETURNING {ROW_COLS}
        "#
    ))
    .bind(id)
    .bind(user_id)
    .bind(new_status)
    .fetch_optional(pool)
    .await
}

/// On sign-in, attach the user's account to any pending invites for their email
/// that have not been claimed yet (across all workspaces).
pub async fn bind_email_to_user(
    pool: &DbPool,
    email_lower: &str,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE workspace_invitations
        SET user_id = $2
        WHERE lower(email) = $1 AND status = 'pending' AND user_id IS NULL
        "#,
    )
    .bind(email_lower)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}
