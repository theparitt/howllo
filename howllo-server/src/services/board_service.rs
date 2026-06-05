use actix_web::HttpRequest;
use serde_json::json;
use uuid::Uuid;

use crate::audit::{self, record_in_tx, AuditEntry};
use crate::auth::{maybe_authenticated_user, require_permission};
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::dto::{BoardDetailDto, BoardListItemDto};
use crate::errors::AppError;
use crate::memberships;
use crate::repositories::board_repository;

async fn require_admin_by_tenant_slug(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id = board_repository::get_tenant_id_by_slug(pool, tenant_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_slug = tenant_slug, "error fetching tenant for board admin check");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::ManageBoards)
        .await
        .map(|ctx| ctx.tenant_id)
}

async fn require_admin_by_board_id(
    pool: &DbPool,
    board_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let tenant_id = board_repository::get_board_tenant(pool, board_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, board_id = %board_id, "error fetching board for admin check");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    require_permission(pool, tenant_id, user_id, Permission::ManageBoards)
        .await
        .map(|_| ())
}

pub async fn list_boards(
    pool: &DbPool,
    tenant_slug: &str,
) -> Result<Vec<BoardListItemDto>, AppError> {
    board_repository::ensure_default_board_for_tenant_slug(pool, tenant_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_slug = tenant_slug, "error ensuring default board");
            AppError::InternalServerError
        })?;

    board_repository::list_public_boards(pool, tenant_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_slug = tenant_slug, "error fetching boards");
            AppError::InternalServerError
        })
}

pub async fn get_board_detail(
    req: &HttpRequest,
    pool: &DbPool,
    tenant_slug: &str,
    board_slug: &str,
) -> Result<BoardDetailDto, AppError> {
    let board = board_repository::get_public_board_by_slug(pool, tenant_slug, board_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_slug = tenant_slug, board_slug = board_slug, "error fetching board detail");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    if board.is_private {
        let user = maybe_authenticated_user(req)
            .await?
            .ok_or(AppError::Forbidden)?;
        let tenant_id = board_repository::get_tenant_id_by_slug_required(pool, tenant_slug)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, tenant_slug = tenant_slug, "error resolving tenant for private board access");
                AppError::InternalServerError
            })?;

        memberships::check_membership(pool, tenant_id, user.id)
            .await
            .map_err(|_| AppError::Forbidden)?;
    }

    Ok(board)
}

pub async fn list_admin_boards(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Vec<BoardDetailDto>, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;

    board_repository::ensure_default_board_for_tenant_slug(pool, tenant_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_slug = tenant_slug, "error ensuring default admin board");
            AppError::InternalServerError
        })?;

    board_repository::list_admin_boards(pool, tenant_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, "error listing admin boards");
            AppError::InternalServerError
        })
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub async fn create_board(
    pool: &DbPool,
    tenant_slug: &str,
    slug: &str,
    name: &str,
    description: Option<&str>,
    board_type: &str,
    is_private: bool,
    icon_url: Option<&str>,
    user_id: Uuid,
) -> Result<BoardDetailDto, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, "error starting board create transaction");
        AppError::InternalServerError
    })?;

    let board = board_repository::create_board(
        &mut tx,
        tenant_id,
        slug,
        name,
        description,
        board_type,
        is_private,
        icon_url,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, "error creating board");
        AppError::InternalServerError
    })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id,
            actor_user_id: user_id,
            entity_type: "board",
            entity_id: board.id,
            action: audit::BOARD_CREATED,
            old_value: None,
            new_value: Some(json!({
                "slug": board.slug,
                "name": board.name,
                "description": board.description,
                "board_type": board.board_type,
                "is_private": board.is_private,
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, "error committing board create transaction");
        AppError::InternalServerError
    })?;

    Ok(board)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_board(
    pool: &DbPool,
    board_id: Uuid,
    name: &str,
    description: Option<&str>,
    board_type: &str,
    is_private: bool,
    icon_url: Option<&str>,
    user_id: Uuid,
) -> Result<BoardDetailDto, AppError> {
    require_admin_by_board_id(pool, board_id, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, board_id = %board_id, "error starting board update transaction");
        AppError::InternalServerError
    })?;

    let previous = board_repository::get_board_for_update(&mut tx, board_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, board_id = %board_id, "error fetching board before update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let board = board_repository::update_board(
        &mut tx,
        board_id,
        name,
        description,
        board_type,
        is_private,
        icon_url,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, board_id = %board_id, "error updating board");
        AppError::InternalServerError
    })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id: previous.tenant_id,
            actor_user_id: user_id,
            entity_type: "board",
            entity_id: board_id,
            action: audit::BOARD_UPDATED,
            old_value: Some(json!({
                "slug": previous.slug,
                "name": previous.name,
                "description": previous.description,
                "board_type": previous.board_type,
                "is_private": previous.is_private,
            })),
            new_value: Some(json!({
                "slug": board.slug,
                "name": board.name,
                "description": board.description,
                "board_type": board.board_type,
                "is_private": board.is_private,
            })),
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, board_id = %board_id, "error committing board update transaction");
        AppError::InternalServerError
    })?;

    Ok(board)
}

pub async fn delete_board(pool: &DbPool, board_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    require_admin_by_board_id(pool, board_id, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, board_id = %board_id, "error starting board delete transaction");
        AppError::InternalServerError
    })?;

    let previous = board_repository::get_board_for_update(&mut tx, board_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, board_id = %board_id, "error fetching board before delete");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    if previous.is_default {
        board_repository::disable_default_board_for_tenant(&mut tx, previous.tenant_id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, tenant_id = %previous.tenant_id, "error disabling default board bootstrap");
                AppError::InternalServerError
            })?;
    }

    board_repository::delete_board(&mut tx, board_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, board_id = %board_id, "error deleting board");
            AppError::InternalServerError
        })?;

    record_in_tx(
        &mut tx,
        AuditEntry {
            tenant_id: previous.tenant_id,
            actor_user_id: user_id,
            entity_type: "board",
            entity_id: board_id,
            action: audit::BOARD_DELETED,
            old_value: Some(json!({
                "slug": previous.slug,
                "name": previous.name,
                "description": previous.description,
                "board_type": previous.board_type,
                "is_private": previous.is_private,
            })),
            new_value: None,
            reason: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, board_id = %board_id, "error committing board delete transaction");
        AppError::InternalServerError
    })?;

    Ok(())
}
