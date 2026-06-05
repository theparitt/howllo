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
    let role = crate::memberships::check_membership(pool, tenant_id, user_id)
        .await
        .map_err(|_| AppError::Forbidden)?;

    Ok(AuthzContext {
        tenant_id,
        user_id,
        role,
    })
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
