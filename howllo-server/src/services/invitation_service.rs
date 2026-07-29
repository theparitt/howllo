use serde_json::json;
use uuid::Uuid;

use crate::audit::{self, record, AuditEntry};
use crate::auth::require_permission;
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::domain::role::Role;
use crate::errors::AppError;
use crate::repositories::{
    invitation_repository::{self, InvitationListItem, InvitationRow},
    membership_repository, notification_repository,
};

fn normalize_email(raw: &str) -> Result<String, AppError> {
    let email = raw.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::Validation("a valid email is required".to_string()));
    }
    Ok(email)
}

/// Staff role that can be invited (owner is reserved / not invitable).
fn parse_invite_role(raw: &str) -> Result<Role, AppError> {
    let role = Role::parse(raw.trim())?;
    if matches!(role, Role::Owner) {
        return Err(AppError::Validation(
            "owner cannot be assigned via invitation".to_string(),
        ));
    }
    Ok(role)
}

pub async fn create_invitation(
    pool: &DbPool,
    tenant_slug: &str,
    actor_user_id: Uuid,
    email: &str,
    role: &str,
) -> Result<InvitationRow, AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, tenant_slug).await?;
    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;

    let email = normalize_email(email)?;
    let role = parse_invite_role(role)?;

    let existing_user = invitation_repository::find_user_id_by_email(pool, &email)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "error looking up invite target user");
            AppError::InternalServerError
        })?;

    let invitation = invitation_repository::create(
        pool,
        tenant_id,
        &email,
        role.as_db_str(),
        actor_user_id,
        existing_user,
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

    Ok(invitation)
}

pub async fn list_invitations(
    pool: &DbPool,
    tenant_slug: &str,
    actor_user_id: Uuid,
) -> Result<Vec<InvitationListItem>, AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, tenant_slug).await?;
    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;

    invitation_repository::list_for_tenant(pool, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, "error listing invitations");
            AppError::InternalServerError
        })
}

pub async fn withdraw_invitation(
    pool: &DbPool,
    tenant_slug: &str,
    actor_user_id: Uuid,
    invitation_id: Uuid,
) -> Result<(), AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, tenant_slug).await?;
    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;

    let invitation = invitation_repository::withdraw(pool, invitation_id, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, invitation_id = %invitation_id, "error withdrawing invitation");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

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
    user_id: Uuid,
) -> Result<Vec<InvitationListItem>, AppError> {
    invitation_repository::list_pending_for_user(pool, user_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, user_id = %user_id, "error listing my invitations");
            AppError::InternalServerError
        })
}

pub async fn accept_invitation(
    pool: &DbPool,
    user_id: Uuid,
    invitation_id: Uuid,
) -> Result<(), AppError> {
    let invitation = invitation_repository::respond(pool, invitation_id, user_id, "accepted")
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
        DO UPDATE SET role = EXCLUDED.role
        WHERE memberships.role <> 'owner'
        "#,
    )
    .bind(invitation.tenant_id)
    .bind(user_id)
    .bind(&invitation.role)
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %invitation.tenant_id, user_id = %user_id, "error granting membership on accept");
        AppError::InternalServerError
    })?;

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

    record(
        pool,
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

    Ok(())
}

pub async fn reject_invitation(
    pool: &DbPool,
    user_id: Uuid,
    invitation_id: Uuid,
) -> Result<(), AppError> {
    let invitation = invitation_repository::respond(pool, invitation_id, user_id, "rejected")
        .await
        .map_err(|error| {
            tracing::error!(error = %error, invitation_id = %invitation_id, "error rejecting invitation");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

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
