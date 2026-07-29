use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::domain::role::Role;
use crate::errors::AppError;

#[derive(Debug, Clone, Copy)]
pub struct AuthzContext {
    pub tenant_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub role: Role,
}

pub async fn require_owner(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<AuthzContext, AppError> {
    require_role(pool, tenant_id, user_id, Role::Owner).await
}

pub async fn require_admin(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<AuthzContext, AppError> {
    let ctx = membership_context(pool, tenant_id, user_id).await?;
    if matches!(ctx.role, Role::Owner | Role::Admin) {
        Ok(ctx)
    } else {
        Err(AppError::Forbidden)
    }
}

pub async fn require_moderator(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<AuthzContext, AppError> {
    let ctx = membership_context(pool, tenant_id, user_id).await?;
    if matches!(ctx.role, Role::Owner | Role::Admin | Role::Moderator) {
        Ok(ctx)
    } else {
        Err(AppError::Forbidden)
    }
}

pub async fn require_member(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<AuthzContext, AppError> {
    membership_context(pool, tenant_id, user_id).await
}

pub async fn require_permission(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
    permission: Permission,
) -> Result<AuthzContext, AppError> {
    let ctx = membership_context(pool, tenant_id, user_id).await?;
    if has_permission(ctx.role, permission) {
        Ok(ctx)
    } else {
        Err(AppError::Forbidden)
    }
}

async fn membership_context(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<AuthzContext, AppError> {
    // Account owners/admins act as workspace owners on every workspace in their
    // account, even without an explicit per-workspace membership. See docs/TENANCY.md.
    if is_account_manager_for_tenant(pool, tenant_id, user_id).await? {
        return Ok(AuthzContext {
            tenant_id,
            user_id,
            role: Role::Owner,
        });
    }

    let role = crate::memberships::check_membership(pool, tenant_id, user_id)
        .await
        .map_err(|_| AppError::Forbidden)?;

    Ok(AuthzContext {
        tenant_id,
        user_id,
        role,
    })
}

/// Whether the user owns/admins the account that this workspace belongs to.
async fn is_account_manager_for_tenant(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<bool, AppError> {
    let found: Option<uuid::Uuid> = sqlx::query_scalar(
        r#"
        SELECT am.account_id
        FROM tenants t
        JOIN account_memberships am ON am.account_id = t.account_id
        WHERE t.id = $1 AND am.user_id = $2 AND am.role IN ('owner', 'admin')
        LIMIT 1
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error checking account manager");
        AppError::InternalServerError
    })?;
    Ok(found.is_some())
}

async fn require_role(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
    role: Role,
) -> Result<AuthzContext, AppError> {
    let ctx = membership_context(pool, tenant_id, user_id).await?;
    if ctx.role == role {
        Ok(ctx)
    } else {
        Err(AppError::Forbidden)
    }
}

fn has_permission(role: Role, permission: Permission) -> bool {
    use Permission::*;
    match role {
        Role::Owner => true,
        Role::Admin => matches!(
            permission,
            ManageBoards
                | ManageTags
                | ManageMembers
                | ManageSettings
                | ManageWebhooks
                | ManageApiTokens
                | ExportData
                | ModerateContent
                | ChangeStatus
                | HidePost
                | HideComment
                | LockPost
                | MarkDuplicate
                | AddModerationNote
                | CreatePost
                | EditOwnPost
                | Vote
                | Comment
                | Follow
        ),
        Role::Moderator => matches!(
            permission,
            ModerateContent
                | HidePost
                | HideComment
                | LockPost
                | MarkDuplicate
                | AddModerationNote
                | Vote
                | Comment
                | Follow
        ),
        Role::Member => matches!(
            permission,
            CreatePost | EditOwnPost | Vote | Comment | Follow
        ),
    }
}
