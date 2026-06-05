use serde_json::json;
use uuid::Uuid;

use crate::audit::{self, record_in_tx, AuditEntry};
use crate::auth::require_permission;
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::domain::role::Role;
use crate::dto::{CreateMemberRequest, MembershipItemDto, UpdateMemberRoleRequest};
use crate::errors::AppError;
use crate::repositories::membership_repository;

pub async fn list_members(
    pool: &DbPool,
    tenant_slug: &str,
    actor_user_id: Uuid,
) -> Result<Vec<MembershipItemDto>, AppError> {
    let tenant_id = membership_repository::get_tenant_id_by_slug(pool, tenant_slug)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_slug = tenant_slug, "error resolving tenant for list members");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;
    membership_repository::list_members(pool, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, "error listing members");
            AppError::InternalServerError
        })
}

pub async fn create_member(
    pool: &DbPool,
    body: &CreateMemberRequest,
    actor_user_id: Uuid,
) -> Result<MembershipItemDto, AppError> {
    let tenant_id = membership_repository::get_tenant_id_by_slug(pool, &body.tenant_slug)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_slug = body.tenant_slug.as_str(), "error resolving tenant for create member");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;
    let role = Role::parse(body.role.trim())?;

    if body.email.trim().is_empty() {
        return Err(AppError::Validation("email is required".to_string()));
    }

    let display_name = body
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| body.email.split('@').next().unwrap_or("Member"));

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, "error starting create member transaction");
        AppError::InternalServerError
    })?;

    let invited_subject = format!("invited:{}", body.email.trim().to_lowercase());
    let email_lower = body.email.trim().to_lowercase();

    let user = membership_repository::upsert_user(
        &mut tx,
        &invited_subject,
        &email_lower,
        display_name,
    )
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, "error upserting member user");
        AppError::InternalServerError
    })?;

    let previous = membership_repository::get_membership_role(pool, tenant_id, user.id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user.id, "error fetching previous membership");
            AppError::InternalServerError
        })?;

    membership_repository::upsert_membership(&mut tx, tenant_id, user.id, role.as_db_str())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user.id, "error upserting membership");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id,
            entity_type: "member",
            entity_id: user.id,
            action: audit::MEMBER_ROLE_CHANGED,
            old_value: previous.map(|row| json!({ "role": row.role })),
            new_value: Some(json!({ "role": role.as_db_str(), "email": user.email, "display_name": user.display_name })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user.id, "error committing create member transaction");
        AppError::InternalServerError
    })?;

    Ok(MembershipItemDto {
        user_id: user.id,
        email: user.email,
        display_name: user.display_name,
        role: role.as_db_str().to_string(),
    })
}

pub async fn update_member_role(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
    body: &UpdateMemberRoleRequest,
    actor_user_id: Uuid,
) -> Result<MembershipItemDto, AppError> {
    let tenant_id = membership_repository::get_tenant_id_by_slug(pool, tenant_slug)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_slug = tenant_slug, "error resolving tenant for update member role");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;
    let role = Role::parse(body.role.trim())?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error starting update member role transaction");
        AppError::InternalServerError
    })?;

    let previous = membership_repository::get_membership(&mut *tx, tenant_id, user_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error fetching membership before update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    membership_repository::update_membership_role(&mut tx, tenant_id, user_id, role.as_db_str())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error updating member role");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id,
            entity_type: "member",
            entity_id: user_id,
            action: audit::MEMBER_ROLE_CHANGED,
            old_value: Some(json!({ "role": previous.role })),
            new_value: Some(json!({ "role": role.as_db_str() })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error committing update member role transaction");
        AppError::InternalServerError
    })?;

    Ok(MembershipItemDto {
        user_id,
        email: previous.email,
        display_name: previous.display_name,
        role: role.as_db_str().to_string(),
    })
}

pub async fn remove_member(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
    actor_user_id: Uuid,
) -> Result<(), AppError> {
    let tenant_id = membership_repository::get_tenant_id_by_slug(pool, tenant_slug)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_slug = tenant_slug, "error resolving tenant for remove member");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error starting remove member transaction");
        AppError::InternalServerError
    })?;

    let previous = membership_repository::get_membership(&mut *tx, tenant_id, user_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error fetching membership before delete");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    membership_repository::delete_membership(&mut tx, tenant_id, user_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error deleting membership");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id,
            entity_type: "member",
            entity_id: user_id,
            action: audit::MEMBER_REMOVED,
            old_value: Some(json!({
                "role": previous.role,
                "email": previous.email,
                "display_name": previous.display_name,
            })),
            new_value: None,
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error committing remove member transaction");
        AppError::InternalServerError
    })?;

    Ok(())
}
