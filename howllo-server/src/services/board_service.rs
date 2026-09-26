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

    if !board.is_enabled {
        return Err(AppError::NotFound);
    }

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

        memberships::require_private_access(pool, tenant_id, user.id).await?;
    }

    Ok(board)
}

pub async fn list_admin_boards(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: Uuid,
) -> Result<Vec<BoardDetailDto>, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;

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
    intro_text: Option<&str>,
    allow_votes: Option<bool>,
    allow_comments: Option<bool>,
    is_private: bool,
    icon_url: Option<&str>,
    background_color: Option<&str>,
    header_image_url: Option<&str>,
    background_image_url: Option<&str>,
    dashboard_sections: Option<Vec<String>>,
    user_id: Uuid,
) -> Result<BoardDetailDto, AppError> {
    let tenant_id = require_admin_by_tenant_slug(pool, tenant_slug, user_id).await?;
    validate_optional_color(background_color, "background_color")?;
    validate_optional_image_url(header_image_url, "header_image_url")?;
    validate_optional_image_url(background_image_url, "background_image_url")?;
    let dashboard_sections = normalize_dashboard_sections(dashboard_sections)?;

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
        intro_text,
        allow_votes.unwrap_or(matches!(board_type, "feature-requests" | "feedback")),
        allow_comments.unwrap_or(true),
        is_private,
        icon_url,
        background_color,
        header_image_url,
        background_image_url,
        &dashboard_sections,
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
                "is_enabled": board.is_enabled,
                "background_color": board.background_color,
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
    intro_text: Option<&str>,
    allow_votes: Option<bool>,
    allow_comments: Option<bool>,
    is_private: bool,
    icon_url: Option<&str>,
    background_color: Option<&str>,
    header_image_url: Option<&str>,
    background_image_url: Option<&str>,
    dashboard_sections: Option<Vec<String>>,
    is_enabled: Option<bool>,
    user_id: Uuid,
) -> Result<BoardDetailDto, AppError> {
    require_admin_by_board_id(pool, board_id, user_id).await?;
    validate_optional_color(background_color, "background_color")?;
    validate_optional_image_url(header_image_url, "header_image_url")?;
    validate_optional_image_url(background_image_url, "background_image_url")?;
    let dashboard_sections = normalize_dashboard_sections(dashboard_sections)?;

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
        intro_text.or(previous.intro_text.as_deref()),
        allow_votes.unwrap_or(previous.allow_votes),
        allow_comments.unwrap_or(previous.allow_comments),
        is_private,
        icon_url,
        background_color,
        header_image_url,
        background_image_url,
        &dashboard_sections,
        is_enabled,
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
                "is_enabled": previous.is_enabled,
                "background_color": previous.background_color,
            })),
            new_value: Some(json!({
                "slug": board.slug,
                "name": board.name,
                "description": board.description,
                "board_type": board.board_type,
                "is_private": board.is_private,
                "is_enabled": board.is_enabled,
                "background_color": board.background_color,
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

    let asset_urls: Vec<String> = sqlx::query_scalar(
        "SELECT url FROM (SELECT jsonb_array_elements_text(attachments) AS url FROM posts WHERE board_id = $1 UNION SELECT icon_url AS url FROM boards WHERE id = $1 UNION SELECT header_image_url AS url FROM boards WHERE id = $1 UNION SELECT background_image_url AS url FROM boards WHERE id = $1) assets WHERE url IS NOT NULL",
    )
    .bind(board_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!(%error, %board_id, "error collecting board assets before delete");
        AppError::InternalServerError
    })?;

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
                "background_color": previous.background_color,
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

    let cleanup_pool = pool.clone();
    let tenant_id = previous.tenant_id;
    tokio::spawn(async move {
        cleanup_deleted_board_assets(&cleanup_pool, tenant_id, asset_urls).await;
    });

    Ok(())
}

async fn cleanup_deleted_board_assets(pool: &DbPool, tenant_id: Uuid, urls: Vec<String>) {
    let cfg = match crate::storage::load_platform_storage_config(pool).await {
        Ok(cfg) => cfg,
        Err(error) => {
            tracing::warn!(%error, %tenant_id, "could not load storage settings for deleted board cleanup");
            return;
        }
    };
    let prefix = format!(
        "{}/workspaces/{tenant_id}/uploads/",
        cfg.public_base_url.trim_end_matches('/')
    );
    for url in urls {
        let Some(file_name) = url.strip_prefix(&prefix) else {
            continue;
        };
        if file_name.is_empty()
            || file_name.contains('/')
            || file_name.contains('\\')
            || file_name.contains("..")
        {
            continue;
        }
        let relative_path = format!("workspaces/{tenant_id}/uploads/{file_name}");
        let owned = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM workspace_assets WHERE tenant_id = $1 AND relative_path = $2)",
        )
        .bind(tenant_id)
        .bind(&relative_path)
        .fetch_one(pool)
        .await;
        if !matches!(owned, Ok(true)) {
            continue;
        }
        let referenced = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM posts WHERE attachments @> jsonb_build_array($1::text)) OR EXISTS(SELECT 1 FROM boards WHERE icon_url = $1 OR header_image_url = $1 OR background_image_url = $1) OR EXISTS(SELECT 1 FROM tenant_branding WHERE logo_url = $1)",
        )
        .bind(&url)
        .fetch_one(pool)
        .await;
        if !matches!(referenced, Ok(false)) {
            continue;
        }
        if let Err(error) = crate::storage::delete_public_asset(pool, &relative_path).await {
            tracing::warn!(%error, %tenant_id, %relative_path, "could not remove orphaned board asset");
            continue;
        }
        if let Err(error) =
            sqlx::query("DELETE FROM workspace_assets WHERE tenant_id = $1 AND relative_path = $2")
                .bind(tenant_id)
                .bind(&relative_path)
                .execute(pool)
                .await
        {
            tracing::warn!(%error, %tenant_id, %relative_path, "could not clear deleted board asset quota");
        }
    }
}

fn validate_optional_image_url(value: Option<&str>, field: &str) -> Result<(), AppError> {
    let Some(value) = value else {
        return Ok(());
    };
    let valid = value.len() <= 2048
        && url::Url::parse(value)
            .map(|url| {
                matches!(url.scheme(), "https" | "http")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
            })
            .unwrap_or(false);
    if valid {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "{field} must be an HTTP(S) image URL"
        )))
    }
}

fn validate_optional_color(value: Option<&str>, field: &str) -> Result<(), AppError> {
    let Some(value) = value else {
        return Ok(());
    };
    let bytes = value.as_bytes();
    let is_valid = bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit());

    if is_valid {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "{field} must be a hex color like #f36949"
        )))
    }
}

/// The dashboard sections a board can show, in canonical display order.
const DASHBOARD_SECTIONS: [&str; 3] = ["progress", "latest", "top"];

/// Validate and normalize the requested dashboard sections. `None` means "use
/// all sections" (default). Unknown values are rejected. The result keeps the
/// canonical order and is deduplicated, so callers/storage stay consistent.
fn normalize_dashboard_sections(requested: Option<Vec<String>>) -> Result<Vec<String>, AppError> {
    let Some(requested) = requested else {
        return Ok(DASHBOARD_SECTIONS.iter().map(|s| s.to_string()).collect());
    };
    for value in &requested {
        if !DASHBOARD_SECTIONS.contains(&value.as_str()) {
            return Err(AppError::Validation(format!(
                "dashboard_sections may only contain: {}",
                DASHBOARD_SECTIONS.join(", ")
            )));
        }
    }
    Ok(DASHBOARD_SECTIONS
        .iter()
        .filter(|section| requested.iter().any(|r| r == *section))
        .map(|s| s.to_string())
        .collect())
}
