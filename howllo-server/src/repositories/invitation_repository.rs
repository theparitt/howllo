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
    pub provider_id: Option<String>,
    pub provider_invitation_id: Option<Uuid>,
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
    pub accepted_user_id: Option<Uuid>,
    pub accepted_user_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub provider_id: Option<String>,
    pub provider_status: Option<String>,
}

const ROW_COLS: &str =
    "id, tenant_id, email, role, status, invited_by, user_id, created_at, responded_at, provider_id, provider_invitation_id";

/// Does a real account already exist for this (lowercased) email? Used to stamp
/// user_id at invite time so the notification/feed can reach them immediately.
pub async fn find_user_id_by_email(
    pool: &DbPool,
    email_lower: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT u.id FROM users u JOIN user_identities i ON i.user_id = u.id AND i.provider_id = 'rooiam' WHERE lower(u.email) = $1 LIMIT 1")
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
    redemption_code_hash: &str,
) -> Result<InvitationRow, sqlx::Error> {
    expire_for_email(pool, tenant_id, email_lower).await?;
    sqlx::query_as::<_, InvitationRow>(&format!(
        r#"
        INSERT INTO workspace_invitations (tenant_id, email, role, invited_by, user_id, expires_at, redemption_code_hash)
        VALUES ($1, $2, $3, $4, $5, NOW() + INTERVAL '7 days', $6)
        RETURNING {ROW_COLS}
        "#
    ))
    .bind(tenant_id)
    .bind(email_lower)
    .bind(role)
    .bind(invited_by)
    .bind(user_id)
    .bind(redemption_code_hash)
    .fetch_one(pool)
    .await
}

pub async fn create_provider(
    pool: &DbPool,
    tenant_id: Uuid,
    email_lower: &str,
    role: &str,
    invited_by: Uuid,
    provider_id: &str,
    provider_invitation_id: Uuid,
) -> Result<InvitationRow, sqlx::Error> {
    expire_for_email(pool, tenant_id, email_lower).await?;
    sqlx::query_as::<_, InvitationRow>(&format!(
        "INSERT INTO workspace_invitations (tenant_id,email,role,invited_by,expires_at,provider_id,provider_invitation_id) VALUES ($1,$2,$3,$4,NOW()+INTERVAL '7 days',$5,$6) RETURNING {ROW_COLS}"
    ))
    .bind(tenant_id).bind(email_lower).bind(role).bind(invited_by).bind(provider_id).bind(provider_invitation_id)
    .fetch_one(pool).await
}

pub async fn pending_for_user(pool: &DbPool, invitation_id: Uuid, user_id: Uuid) -> Result<Option<InvitationRow>, sqlx::Error> {
    sqlx::query_as::<_, InvitationRow>(&format!("SELECT {ROW_COLS} FROM workspace_invitations WHERE id=$1 AND user_id=$2 AND status='pending' AND COALESCE(expires_at,created_at+INTERVAL '7 days')>NOW()"))
        .bind(invitation_id).bind(user_id).fetch_optional(pool).await
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
    sqlx::query(
        "UPDATE workspace_invitations SET status = 'expired', responded_at = NOW() WHERE tenant_id = $1 AND status = 'pending' AND COALESCE(expires_at, created_at + INTERVAL '7 days') <= NOW()",
    )
    .bind(tenant_id)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, InvitationListItem>(
        r#"
        SELECT i.id, i.email, i.role, i.status,
               t.slug AS tenant_slug, t.name AS tenant_name,
               u.display_name AS invited_by_name,
               CASE WHEN i.status = 'accepted' THEN i.user_id END AS accepted_user_id,
               CASE WHEN i.status = 'accepted' THEN accepted_user.display_name END AS accepted_user_name,
               i.created_at, i.provider_id, NULL::text AS provider_status
        FROM workspace_invitations i
        JOIN tenants t ON t.id = i.tenant_id
        LEFT JOIN users u ON u.id = i.invited_by
        LEFT JOIN users accepted_user ON accepted_user.id = i.user_id
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
    sqlx::query(
        "UPDATE workspace_invitations SET status = 'expired', responded_at = NOW() WHERE user_id = $1 AND status = 'pending' AND COALESCE(expires_at, created_at + INTERVAL '7 days') <= NOW()",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, InvitationListItem>(
        r#"
        SELECT i.id, i.email, i.role, i.status,
               t.slug AS tenant_slug, t.name AS tenant_name,
               u.display_name AS invited_by_name,
               NULL::uuid AS accepted_user_id,
               NULL::text AS accepted_user_name,
               i.created_at, i.provider_id, NULL::text AS provider_status
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
          AND COALESCE(expires_at, created_at + INTERVAL '7 days') > NOW()
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
          AND COALESCE(expires_at, created_at + INTERVAL '7 days') > NOW()
        RETURNING {ROW_COLS}
        "#
    ))
    .bind(id)
    .bind(user_id)
    .bind(new_status)
    .fetch_optional(pool)
    .await
}

/// The acceptance path uses one transaction for the invitation transition,
/// membership grant and audit record.
pub async fn respond_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    user_id: Uuid,
    new_status: &str,
) -> Result<Option<InvitationRow>, sqlx::Error> {
    sqlx::query_as::<_, InvitationRow>(&format!(
        r#"
        UPDATE workspace_invitations
        SET status = $3, responded_at = NOW()
        WHERE id = $1 AND user_id = $2 AND status = 'pending'
          AND COALESCE(expires_at, created_at + INTERVAL '7 days') > NOW()
        RETURNING {ROW_COLS}
        "#
    ))
    .bind(id)
    .bind(user_id)
    .bind(new_status)
    .fetch_optional(&mut **tx)
    .await
}

pub async fn reject_code_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    code_hash: &str,
    user_id: Uuid,
) -> Result<Option<InvitationRow>, sqlx::Error> {
    sqlx::query_as::<_, InvitationRow>(&format!(
        "UPDATE workspace_invitations SET status='rejected', responded_at=NOW(), user_id=$2 WHERE redemption_code_hash=$1 AND status='pending' AND COALESCE(expires_at, created_at + INTERVAL '7 days') > NOW() RETURNING {ROW_COLS}"
    ))
    .bind(code_hash)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
}

/// Expire a pending invitation before acting on it. A pre-expiry invitation
/// created before `expires_at` was populated still has a seven-day lifetime.
pub async fn expire_for_id_in_tenant(
    pool: &DbPool,
    id: Uuid,
    tenant_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE workspace_invitations SET status = 'expired', responded_at = NOW() WHERE id = $1 AND tenant_id = $2 AND status = 'pending' AND COALESCE(expires_at, created_at + INTERVAL '7 days') <= NOW()",
    )
    .bind(id)
    .bind(tenant_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn expire_for_id_for_user(
    pool: &DbPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE workspace_invitations SET status = 'expired', responded_at = NOW() WHERE id = $1 AND user_id = $2 AND status = 'pending' AND COALESCE(expires_at, created_at + INTERVAL '7 days') <= NOW()",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn expire_for_email(
    pool: &DbPool,
    tenant_id: Uuid,
    email_lower: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE workspace_invitations SET status = 'expired', responded_at = NOW() WHERE tenant_id = $1 AND lower(email) = $2 AND status = 'pending' AND COALESCE(expires_at, created_at + INTERVAL '7 days') <= NOW()",
    )
    .bind(tenant_id)
    .bind(email_lower)
    .execute(pool)
    .await?;
    Ok(())
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
