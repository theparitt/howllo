use argon2::password_hash::rand_core::{OsRng, RngCore};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::str::FromStr;
use uuid::Uuid;

use crate::audit::{self, record, AuditEntry};
use crate::auth::require_permission;
use crate::db::DbPool;
use crate::config::Settings;
use crate::domain::permission::Permission;
use crate::domain::role::Role;
use crate::errors::AppError;
use crate::repositories::{
    invitation_repository::{self, InvitationListItem, InvitationRow},
    membership_repository, notification_repository,
};

fn normalize_email(raw: &str) -> Result<String, AppError> {
    let email = raw.trim().to_lowercase();
    if lettre::Address::from_str(&email).is_err() {
        return Err(AppError::Validation(
            "a valid email is required".to_string(),
        ));
    }
    Ok(email)
}

/// Staff role that can be invited (owner is reserved / not invitable).
fn parse_invite_role(raw: &str) -> Result<Role, AppError> {
    let role = Role::parse(raw.trim())?;
    if !matches!(role, Role::Admin | Role::Moderator) {
        return Err(AppError::Validation(
            "staff invitations support admin or moderator roles".to_string(),
        ));
    }
    Ok(role)
}

pub async fn create_invitation(
    pool: &DbPool,
    settings: &Settings,
    tenant_slug: &str,
    actor_user_id: Uuid,
    email: &str,
    role: &str,
) -> Result<CreatedInvitation, AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, tenant_slug).await?;
    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;

    let email = normalize_email(email)?;
    let role = parse_invite_role(role)?;
    let attempts: i32 = sqlx::query_scalar("INSERT INTO invitation_email_limits(tenant_id,actor_user_id,window_start,attempts) VALUES($1,$2,NOW(),1) ON CONFLICT(tenant_id,actor_user_id) DO UPDATE SET attempts=CASE WHEN invitation_email_limits.window_start<NOW()-INTERVAL '1 day' THEN 1 ELSE invitation_email_limits.attempts+1 END,window_start=CASE WHEN invitation_email_limits.window_start<NOW()-INTERVAL '1 day' THEN NOW() ELSE invitation_email_limits.window_start END RETURNING attempts")
        .bind(tenant_id).bind(actor_user_id).fetch_one(pool).await.map_err(|error| {
            tracing::error!(%error, "invitation limit check failed"); AppError::InternalServerError
        })?;
    if attempts > 20 {
        return Err(AppError::TooManyRequests(
            "Daily invitation limit reached.".into(),
        ));
    }

    if crate::external_identity::linked_workspace(pool, settings, tenant_id).await? {
        let duplicate: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_invitations WHERE tenant_id=$1 AND lower(email)=$2 AND status='pending' AND COALESCE(expires_at,created_at+INTERVAL '7 days')>NOW())")
            .bind(tenant_id).bind(&email).fetch_one(pool).await
            .map_err(|_| AppError::InternalServerError)?;
        if duplicate { return Err(AppError::BadRequest("there is already a pending invitation for this email".into())); }
        let provider_id = crate::external_identity::provider_id(settings)?;
        let sent = crate::external_identity::call_bridge(
            settings, reqwest::Method::POST,
            &format!("/v1/workspaces/{tenant_id}/invitations"), None,
            Some(&json!({"email": email})),
        ).await?;
        let provider_invitation_id = sent.get("id").and_then(|id| id.as_str())
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| AppError::ServiceUnavailable("Identity provider returned an invalid invitation.".into()))?;
        let invitation = match invitation_repository::create_provider(
            pool, tenant_id, &email, role.as_db_str(), actor_user_id,
            provider_id, provider_invitation_id,
        ).await {
            Ok(invitation) => invitation,
            Err(error) => {
                let already_recorded: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_invitations WHERE provider_id=$1 AND provider_invitation_id=$2)")
                    .bind(provider_id).bind(provider_invitation_id).fetch_one(pool).await.unwrap_or(true);
                if !already_recorded {
                    let _ = crate::external_identity::call_bridge(settings, reqwest::Method::DELETE,
                        &format!("/v1/workspaces/{tenant_id}/invitations/{provider_invitation_id}"), None, None).await;
                }
                if let sqlx::Error::Database(db) = &error {
                    if db.is_unique_violation() { return Err(AppError::BadRequest("there is already a pending invitation for this email".into())); }
                }
                tracing::error!(%error, "could not save linked staff invitation");
                return Err(AppError::InternalServerError);
            }
        };
        record(pool, AuditEntry { tenant_id, actor_user_id, entity_type: "invitation",
            entity_id: invitation.id, action: audit::INVITATION_CREATED, old_value: None,
            new_value: Some(json!({"email": email, "role": role.as_db_str(), "provider_id": provider_id})), reason: None }).await?;
        return Ok(CreatedInvitation { invitation, redemption_code: String::new() });
    }

    let existing_user = invitation_repository::find_user_id_by_email(pool, &email)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "error looking up invite target user");
            AppError::InternalServerError
        })?;

    let mut random = [0u8; 32];
    OsRng.fill_bytes(&mut random);
    let redemption_code = format!("howllo_inv_{}", URL_SAFE_NO_PAD.encode(random));
    let code_hash = hex::encode(Sha256::digest(redemption_code.as_bytes()));
    let invitation = invitation_repository::create(
        pool,
        tenant_id,
        &email,
        role.as_db_str(),
        actor_user_id,
        existing_user,
        &code_hash,
    )
    .await
    .map_err(|error| {
        if let sqlx::Error::Database(db) = &error {
            if db.is_unique_violation() {
                return AppError::BadRequest(
                    "there is already a pending invitation for this email".to_string(),
                );
            }
        }
        tracing::error!(error = %error, tenant_id = %tenant_id, "error creating invitation");
        AppError::InternalServerError
    })?;

    // Notify the invitee if they already have an account.
    if let Some(user_id) = existing_user {
        let _ = notification_repository::create_notification(
            pool,
            tenant_id,
            user_id,
            "invitation_received",
            "You've been invited",
            &format!("You were invited to join as {}.", role.as_db_str()),
            None,
        )
        .await;
    }

    record(
        pool,
        AuditEntry {
            tenant_id,
            actor_user_id,
            entity_type: "invitation",
            entity_id: invitation.id,
            action: audit::INVITATION_CREATED,
            old_value: None,
            new_value: Some(json!({ "email": email, "role": role.as_db_str() })),
            reason: None,
        },
    )
    .await?;

    // Email is optional. An invitation remains available in-app when disabled
    // or when the mail queue cannot accept a message.
    let workspace_name: String = sqlx::query_scalar("SELECT name FROM tenants WHERE id=$1")
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| tenant_slug.to_string());
    if let Err(error) = crate::email::delivery::enqueue_invitation(
        pool,
        tenant_id,
        invitation.id,
        &email,
        &workspace_name,
    )
    .await
    {
        tracing::warn!(%error, invitation_id = %invitation.id, "invitation email could not be queued");
    }

    Ok(CreatedInvitation {
        invitation,
        redemption_code,
    })
}

pub struct CreatedInvitation {
    pub invitation: InvitationRow,
    pub redemption_code: String,
}

pub async fn list_invitations(
    pool: &DbPool,
    settings: &Settings,
    tenant_slug: &str,
    actor_user_id: Uuid,
) -> Result<Vec<InvitationListItem>, AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, tenant_slug).await?;
    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;

    let mut items = invitation_repository::list_for_tenant(pool, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, "error listing invitations");
            AppError::InternalServerError
        })?;
    for item in &mut items {
        if item.status != "pending" || item.provider_id.is_none() { continue; }
        let row = invitation_repository::get(pool, item.id).await
            .map_err(|_| AppError::InternalServerError)?.ok_or(AppError::NotFound)?;
        let status = refresh_provider_invitation(pool, settings, &row).await?;
        item.provider_status = Some(status.clone());
        item.status = match status.as_str() { "declined" => "rejected", "revoked" => "withdrawn", "expired" => "expired", _ => "pending" }.into();
    }
    Ok(items)
}

pub async fn withdraw_invitation(
    pool: &DbPool,
    settings: &Settings,
    tenant_slug: &str,
    actor_user_id: Uuid,
    invitation_id: Uuid,
) -> Result<(), AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, tenant_slug).await?;
    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;

    invitation_repository::expire_for_id_in_tenant(pool, invitation_id, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, invitation_id = %invitation_id, "error expiring invitation");
            AppError::InternalServerError
        })?;

    let pending = invitation_repository::get(pool, invitation_id).await
        .map_err(|_| AppError::InternalServerError)?
        .filter(|row| row.tenant_id == tenant_id && row.status == "pending")
        .ok_or(AppError::NotFound)?;
    if let Some(provider_invitation_id) = pending.provider_invitation_id {
        if refresh_provider_invitation(pool, settings, &pending).await? != "pending" {
            return Err(AppError::BadRequest("This invitation can no longer be withdrawn.".into()));
        }
        crate::external_identity::call_bridge(settings, reqwest::Method::DELETE,
            &format!("/v1/workspaces/{tenant_id}/invitations/{provider_invitation_id}"), None, None).await?;
    }

    let invitation = invitation_repository::withdraw(pool, invitation_id, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, invitation_id = %invitation_id, "error withdrawing invitation");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;
    cancel_invitation_email(pool, invitation.id).await;

    if let Some(user_id) = invitation.user_id {
        let _ = notification_repository::create_notification(
            pool,
            tenant_id,
            user_id,
            "invitation_withdrawn",
            "Invitation withdrawn",
            "An invitation to join a workspace was withdrawn.",
            None,
        )
        .await;
    }

    record(
        pool,
        AuditEntry {
            tenant_id,
            actor_user_id,
            entity_type: "invitation",
            entity_id: invitation.id,
            action: audit::INVITATION_WITHDRAWN,
            old_value: None,
            new_value: None,
            reason: None,
        },
    )
    .await?;

    Ok(())
}

pub async fn list_my_invitations(
    pool: &DbPool,
    settings: &Settings,
    user_id: Uuid,
) -> Result<Vec<InvitationListItem>, AppError> {
    // A RooIAM staff member may first visit the App account home without
    // opening a workspace session. Bind their pending invitations here.
    let email: Option<String> = sqlx::query_scalar(
        "SELECT u.email FROM users u JOIN user_identities i ON i.user_id = u.id WHERE u.id = $1 AND i.provider_id = 'rooiam' LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| { tracing::error!(%error, "error loading invitee identity"); AppError::InternalServerError })?;
    if let Some(email) = email.filter(|value| !value.trim().is_empty()) {
        invitation_repository::bind_email_to_user(pool, &email.trim().to_lowercase(), user_id)
            .await
            .map_err(|error| {
                tracing::error!(%error, "error binding staff invitation");
                AppError::InternalServerError
            })?;
    }
    let mut items = invitation_repository::list_pending_for_user(pool, user_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, user_id = %user_id, "error listing my invitations");
            AppError::InternalServerError
        })?;
    let mut visible = Vec::with_capacity(items.len());
    for mut item in items.drain(..) {
        if item.provider_id.is_some() {
            let row = invitation_repository::get(pool, item.id).await
                .map_err(|_| AppError::InternalServerError)?.ok_or(AppError::NotFound)?;
            let status = refresh_provider_invitation(pool, settings, &row).await?;
            if !matches!(status.as_str(), "pending" | "accepted") { continue; }
            item.provider_status = Some(status);
        }
        visible.push(item);
    }
    Ok(visible)
}

async fn refresh_provider_invitation(pool: &DbPool, settings: &Settings, row: &InvitationRow) -> Result<String, AppError> {
    let provider_invitation_id = row.provider_invitation_id.ok_or(AppError::InternalServerError)?;
    if !crate::external_identity::linked_workspace(pool, settings, row.tenant_id).await? {
        return Err(AppError::ServiceUnavailable("Identity provider is not linked to this workspace.".into()));
    }
    let detail = crate::external_identity::call_bridge(settings, reqwest::Method::GET,
        &format!("/v1/workspaces/{}/invitations/{provider_invitation_id}", row.tenant_id), None, None).await?;
    if detail.get("id").and_then(|value| value.as_str()) != Some(provider_invitation_id.to_string().as_str())
        || detail.get("email").and_then(|value| value.as_str())
            .is_none_or(|email| !email.eq_ignore_ascii_case(&row.email)) {
        return Err(AppError::ServiceUnavailable("Identity provider returned inconsistent invitation data.".into()));
    }
    let status = detail.get("status").and_then(|value| value.as_str())
        .ok_or_else(|| AppError::ServiceUnavailable("Identity provider returned an invalid invitation.".into()))?;
    if !matches!(status, "pending" | "accepted" | "declined" | "revoked" | "expired") {
        return Err(AppError::ServiceUnavailable("Identity provider returned an invalid invitation status.".into()));
    }
    let local_status = match status { "declined" => Some("rejected"), "revoked" => Some("withdrawn"), "expired" => Some("expired"), _ => None };
    if let Some(local_status) = local_status {
        let changed = sqlx::query("UPDATE workspace_invitations SET status=$2,responded_at=NOW() WHERE id=$1 AND status='pending'")
            .bind(row.id).bind(local_status).execute(pool).await
            .map_err(|_| AppError::InternalServerError)?.rows_affected() > 0;
        if changed && status == "declined" {
            if let Some(inviter) = row.invited_by {
                let _ = notification_repository::create_notification(pool, row.tenant_id, inviter,
                    "invitation_rejected", "Invitation declined",
                    &format!("{} declined your invitation.", row.email), None).await;
            }
        }
    }
    Ok(status.to_string())
}

pub async fn accept_invitation(
    pool: &DbPool,
    settings: &Settings,
    user_id: Uuid,
    invitation_id: Uuid,
) -> Result<(), AppError> {
    invitation_repository::expire_for_id_for_user(pool, invitation_id, user_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, invitation_id = %invitation_id, "error expiring invitation");
            AppError::InternalServerError
        })?;
    if let Some(invitation) = invitation_repository::pending_for_user(pool, invitation_id, user_id)
        .await.map_err(|_| AppError::InternalServerError)? {
        let linked = crate::external_identity::linked_workspace(pool, settings, invitation.tenant_id).await?;
        if linked && invitation.provider_id.is_none() { return Err(AppError::Forbidden); }
        if let (Some(provider_id), Some(provider_invitation_id)) =
            (&invitation.provider_id, invitation.provider_invitation_id) {
            verify_provider_invitation(pool, settings, &invitation, provider_id, provider_invitation_id, user_id).await?;
        }
    }
    accept_bound_invitation(pool, user_id, invitation_id, None).await
}

async fn verify_provider_invitation(pool: &DbPool, settings: &Settings, invitation: &InvitationRow,
    provider_id: &str, provider_invitation_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    if crate::external_identity::provider_id(settings)? != provider_id
        || !crate::external_identity::linked_workspace(pool, settings, invitation.tenant_id).await? {
        return Err(AppError::ServiceUnavailable("Identity provider is not linked to this workspace.".into()));
    }
    let subject: String = sqlx::query_scalar("SELECT subject FROM user_identities WHERE user_id=$1 AND provider_id=$2")
        .bind(user_id).bind(provider_id).fetch_optional(pool).await
        .map_err(|_| AppError::InternalServerError)?.ok_or(AppError::Forbidden)?;
    let subject_segment = crate::external_identity::encoded_subject(&subject)?;
    let provider_invitation = crate::external_identity::call_bridge(settings, reqwest::Method::GET,
        &format!("/v1/workspaces/{}/invitations/{provider_invitation_id}", invitation.tenant_id), None, None).await?;
    if provider_invitation.get("id").and_then(|value| value.as_str()) != Some(provider_invitation_id.to_string().as_str())
        || provider_invitation.get("email").and_then(|value| value.as_str())
            .is_none_or(|email| !email.eq_ignore_ascii_case(&invitation.email)) {
        return Err(AppError::ServiceUnavailable("Identity provider returned inconsistent invitation data.".into()));
    }
    if !provider_invitation_allows(&provider_invitation, &subject) {
        return Err(AppError::Forbidden);
    }
    let member = crate::external_identity::call_bridge(settings, reqwest::Method::GET,
        &format!("/v1/workspaces/{}/subjects/{subject_segment}", invitation.tenant_id), None, None).await?;
    if member.get("subject").and_then(|value| value.as_str()) != Some(subject.as_str())
        || member.get("status").and_then(|value| value.as_str()) != Some("active") {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

fn provider_invitation_allows(invitation: &serde_json::Value, subject: &str) -> bool {
    invitation.get("status").and_then(|value| value.as_str()) == Some("accepted")
        && invitation.get("accepted_user_id").and_then(|value| value.as_str()) == Some(subject)
}

#[cfg(test)]
mod provider_invitation_tests {
    use super::provider_invitation_allows;
    use serde_json::json;

    #[test]
    fn only_the_verified_accepting_subject_can_activate_staff_access() {
        let active = json!({"status":"accepted", "accepted_user_id":"user-a"});
        assert!(provider_invitation_allows(&active, "user-a"));
        assert!(!provider_invitation_allows(&active, "user-b"));
        assert!(!provider_invitation_allows(&json!({"status":"pending", "accepted_user_id":"user-a"}), "user-a"));
        assert!(!provider_invitation_allows(&json!({"status":"declined", "accepted_user_id":"user-a"}), "user-a"));
        assert!(!provider_invitation_allows(&json!({"status":"accepted"}), "user-a"));
    }
}

pub async fn redeem_invitation(pool: &DbPool, settings: &Settings, user_id: Uuid, code: &str) -> Result<(), AppError> {
    let code_hash = validate_invitation_code(pool, user_id, code).await?;
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM workspace_invitations WHERE redemption_code_hash=$1 AND status='pending' AND COALESCE(expires_at,created_at+INTERVAL '7 days')>NOW()")
        .bind(&code_hash).fetch_optional(pool).await.map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound)?;
    if crate::external_identity::linked_workspace(pool, settings, tenant_id).await? { return Err(AppError::Forbidden); }
    accept_bound_invitation(pool, user_id, Uuid::nil(), Some(&code_hash)).await
}

async fn validate_invitation_code(pool: &DbPool, user_id: Uuid, code: &str) -> Result<String, AppError> {
    let code = code.trim();
    if code.len() != 54 || !code.starts_with("howllo_inv_") {
        return Err(AppError::NotFound);
    }
    crate::auth::local_user::limit_attempt(pool, "invitation:redeem", &user_id.to_string(), 10)
        .await?;
    Ok(hex::encode(Sha256::digest(code.as_bytes())))
}

pub async fn decline_invitation_code(
    pool: &DbPool,
    user_id: Uuid,
    code: &str,
) -> Result<(), AppError> {
    let code_hash = validate_invitation_code(pool, user_id, code).await?;
    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(%error, "error starting invitation decline");
        AppError::InternalServerError
    })?;
    let invitation = invitation_repository::reject_code_in_tx(&mut tx, &code_hash, user_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, "error declining invitation by code");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;
    audit::record_in_tx(&mut tx, AuditEntry {
        tenant_id: invitation.tenant_id,
        actor_user_id: user_id,
        entity_type: "invitation",
        entity_id: invitation.id,
        action: audit::INVITATION_REJECTED,
        old_value: None,
        new_value: None,
        reason: None,
    }).await?;
    tx.commit().await.map_err(|error| {
        tracing::error!(%error, "error committing invitation decline");
        AppError::InternalServerError
    })?;
    cancel_invitation_email(pool, invitation.id).await;
    if let Some(inviter) = invitation.invited_by {
        let _ = notification_repository::create_notification(
            pool,
            invitation.tenant_id,
            inviter,
            "invitation_rejected",
            "Invitation declined",
            &format!("{} declined your invitation.", invitation.email),
            None,
        ).await;
    }
    Ok(())
}

async fn accept_bound_invitation(
    pool: &DbPool,
    user_id: Uuid,
    invitation_id: Uuid,
    code_hash: Option<&str>,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(%error, invitation_id = %invitation_id, "error starting invitation acceptance");
        AppError::InternalServerError
    })?;
    let invitation_id = if let Some(code_hash) = code_hash {
        sqlx::query_scalar::<_, Uuid>(
            "UPDATE workspace_invitations SET user_id=$1 WHERE redemption_code_hash=$2 AND status='pending' AND COALESCE(expires_at, created_at + INTERVAL '7 days') > NOW() RETURNING id"
        )
        .bind(user_id)
        .bind(code_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| { tracing::error!(%error, "error claiming staff invitation"); AppError::InternalServerError })?
        .ok_or(AppError::NotFound)?
    } else {
        invitation_id
    };
    let invitation = invitation_repository::respond_in_tx(&mut tx, invitation_id, user_id, "accepted")
        .await
        .map_err(|error| {
            tracing::error!(error = %error, invitation_id = %invitation_id, "error accepting invitation");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    // Grant the workspace membership, but never demote an existing owner.
    sqlx::query(
        r#"
        INSERT INTO memberships (tenant_id, user_id, role)
        VALUES ($1, $2, $3)
        ON CONFLICT (tenant_id, user_id)
        DO UPDATE SET role = EXCLUDED.role, public_participant = FALSE
        WHERE memberships.role <> 'owner'
        "#,
    )
    .bind(invitation.tenant_id)
    .bind(user_id)
    .bind(&invitation.role)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %invitation.tenant_id, user_id = %user_id, "error granting membership on accept");
        AppError::InternalServerError
    })?;

    audit::record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id: invitation.tenant_id,
            actor_user_id: user_id,
            entity_type: "invitation",
            entity_id: invitation.id,
            action: audit::INVITATION_ACCEPTED,
            old_value: None,
            new_value: Some(json!({ "role": invitation.role })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(%error, invitation_id = %invitation_id, "error committing invitation acceptance");
        AppError::InternalServerError
    })?;
    cancel_invitation_email(pool, invitation.id).await;

    if let Some(inviter) = invitation.invited_by {
        let _ = notification_repository::create_notification(
            pool,
            invitation.tenant_id,
            inviter,
            "invitation_accepted",
            "Invitation accepted",
            &format!("{} accepted your invitation.", invitation.email),
            None,
        )
        .await;
    }

    Ok(())
}

pub async fn reject_invitation(
    pool: &DbPool,
    settings: &Settings,
    user_id: Uuid,
    invitation_id: Uuid,
) -> Result<(), AppError> {
    invitation_repository::expire_for_id_for_user(pool, invitation_id, user_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, invitation_id = %invitation_id, "error expiring invitation");
            AppError::InternalServerError
        })?;
    if let Some(pending) = invitation_repository::pending_for_user(pool, invitation_id, user_id)
        .await.map_err(|_| AppError::InternalServerError)? {
        if let Some(provider_invitation_id) = pending.provider_invitation_id {
            if refresh_provider_invitation(pool, settings, &pending).await? != "pending" {
                return Err(AppError::BadRequest("This invitation can no longer be declined here.".into()));
            }
            crate::external_identity::call_bridge(settings, reqwest::Method::DELETE,
                &format!("/v1/workspaces/{}/invitations/{provider_invitation_id}", pending.tenant_id), None, None).await?;
        }
    }
    let invitation = invitation_repository::respond(pool, invitation_id, user_id, "rejected")
        .await
        .map_err(|error| {
            tracing::error!(error = %error, invitation_id = %invitation_id, "error rejecting invitation");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;
    cancel_invitation_email(pool, invitation.id).await;

    if let Some(inviter) = invitation.invited_by {
        let _ = notification_repository::create_notification(
            pool,
            invitation.tenant_id,
            inviter,
            "invitation_rejected",
            "Invitation declined",
            &format!("{} declined your invitation.", invitation.email),
            None,
        )
        .await;
    }

    record(
        pool,
        AuditEntry {
            tenant_id: invitation.tenant_id,
            actor_user_id: user_id,
            entity_type: "invitation",
            entity_id: invitation.id,
            action: audit::INVITATION_REJECTED,
            old_value: None,
            new_value: None,
            reason: None,
        },
    )
    .await?;

    Ok(())
}

async fn cancel_invitation_email(pool: &DbPool, invitation_id: Uuid) {
    if let Err(error) = sqlx::query("UPDATE email_messages SET status='cancelled',body='' WHERE dedupe_key=$1 AND status='queued'")
        .bind(format!("invitation:{invitation_id}"))
        .execute(pool).await {
        tracing::warn!(%error, %invitation_id, "could not cancel pending invitation email");
    }
}
