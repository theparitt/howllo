use actix_web::HttpRequest;
use uuid::Uuid;

use crate::auth::{maybe_authenticated_user, require_permission};
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::dto::{
    PaginatedResponse, PostCreatedDto, PostDetailDto, PostFollowStateDto, PostListItemDto,
    UpdatePostRequest,
};
use crate::errors::AppError;
use crate::memberships;
use crate::repositories::post_repository;
use crate::statuses::FeedbackStatus;

pub struct BoardPostListInput<'a> {
    pub req: &'a HttpRequest,
    pub pool: &'a DbPool,
    pub board_slug: &'a str,
    pub tenant_slug: &'a str,
    pub sort: &'a str,
    pub status: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub page: i64,
    pub per_page: i64,
    pub include_hidden: bool,
}

pub struct PostInteractionAccess {
    pub tenant_id: Uuid,
}

pub async fn ensure_board_access(
    req: &HttpRequest,
    pool: &DbPool,
    tenant_slug: &str,
    board_slug: &str,
) -> Result<post_repository::BoardAccessRecord, AppError> {
    let board = post_repository::find_board_access(pool, tenant_slug, board_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_slug = tenant_slug, board_slug = board_slug, "error fetching board access metadata");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    if board.is_private {
        let user = maybe_authenticated_user(req)
            .await?
            .ok_or(AppError::Forbidden)?;
        memberships::check_membership(pool, board.tenant_id, user.id)
            .await
            .map_err(|_| AppError::Forbidden)?;
    }

    Ok(board)
}

pub async fn ensure_post_read_access(
    req: &HttpRequest,
    pool: &DbPool,
    post_id: Uuid,
    tenant_slug: &str,
) -> Result<post_repository::PostAccessRecord, AppError> {
    let post = post_repository::find_post_access(pool, post_id, Some(tenant_slug))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, tenant_slug = tenant_slug, "error checking post read access");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    if post.is_hidden || post.deleted_at.is_some() {
        return Err(AppError::NotFound);
    }

    if post.is_private {
        let user = maybe_authenticated_user(req)
            .await?
            .ok_or(AppError::Forbidden)?;
        memberships::check_membership(pool, post.tenant_id, user.id)
            .await
            .map_err(|_| AppError::Forbidden)?;
    }

    Ok(post)
}

pub async fn list_board_posts(
    input: BoardPostListInput<'_>,
) -> Result<PaginatedResponse<PostListItemDto>, AppError> {
    if let Some(status) = input.status {
        FeedbackStatus::parse(status)?;
    }

    let board =
        ensure_board_access(input.req, input.pool, input.tenant_slug, input.board_slug).await?;

    if input.include_hidden {
        let user = maybe_authenticated_user(input.req)
            .await?
            .ok_or(AppError::Unauthorized)?;
        require_permission(
            input.pool,
            board.tenant_id,
            user.id,
            Permission::ModerateContent,
        )
        .await?;
    }

    let status = input
        .status
        .map(FeedbackStatus::parse)
        .transpose()?
        .map(FeedbackStatus::as_db_str);

    let total = post_repository::count_board_posts(
        input.pool,
        input.tenant_slug,
        input.board_slug,
        status,
        input.tag,
        input.include_hidden,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, tenant_slug = input.tenant_slug, board_slug = input.board_slug, "error counting board posts");
        AppError::InternalServerError
    })?;

    let items = post_repository::list_board_posts(
        input.pool,
        post_repository::BoardPostListQuery {
            tenant_slug: input.tenant_slug,
            board_slug: input.board_slug,
            sort: input.sort,
            status,
            tag: input.tag,
            include_hidden: input.include_hidden,
            limit: input.per_page,
            offset: (input.page - 1) * input.per_page,
        },
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, tenant_slug = input.tenant_slug, board_slug = input.board_slug, "error listing board posts");
        AppError::InternalServerError
    })?;

    Ok(PaginatedResponse {
        has_next: input.page * input.per_page < total,
        items,
        page: input.page,
        per_page: input.per_page,
        total,
    })
}

pub async fn get_post_detail(
    req: &HttpRequest,
    pool: &DbPool,
    post_id: Uuid,
    tenant_slug: &str,
) -> Result<PostDetailDto, AppError> {
    ensure_post_read_access(req, pool, post_id, tenant_slug).await?;

    let detail = post_repository::find_post_detail(pool, post_id, tenant_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, tenant_slug = tenant_slug, "error fetching post detail");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let tags = post_repository::get_post_tags(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, "error fetching post tags");
            AppError::InternalServerError
        })?;

    let comment_count = post_repository::get_visible_comment_count(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, "error counting post comments");
            AppError::InternalServerError
        })?;

    let official_response = post_repository::get_official_response(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, "error fetching official response");
            AppError::InternalServerError
        })?;

    let follow_state = match maybe_authenticated_user(req).await? {
        Some(user) => post_repository::get_follow_state(pool, post_id, user.id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, post_id = %post_id, user_id = %user.id, "error fetching follow state for post detail");
                AppError::InternalServerError
            })?
            .or(Some(PostFollowStateDto {
                is_following: false,
                notify_on_status_change: false,
                notify_on_official_response: false,
            })),
        None => None,
    };

    Ok(PostDetailDto {
        author_display_name: detail.author_display_name,
        board_slug: detail.board_slug,
        body: detail.body,
        comment_count,
        created_at: detail.created_at,
        duplicate_of_post_id: detail.duplicate_of_post_id,
        follow_state,
        id: detail.id,
        is_locked: detail.is_locked,
        official_response,
        status: detail.status,
        tags,
        title: detail.title,
        vote_count: detail.vote_count,
        attachments: detail.attachments,
    })
}

pub async fn get_post_status_history(
    req: &HttpRequest,
    pool: &DbPool,
    post_id: Uuid,
    tenant_slug: &str,
) -> Result<Vec<crate::dto::StatusHistoryItemDto>, AppError> {
    ensure_post_read_access(req, pool, post_id, tenant_slug).await?;
    post_repository::get_status_history(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, tenant_slug = tenant_slug, "error fetching status history");
            AppError::InternalServerError
        })
}

#[allow(clippy::too_many_arguments)]
pub async fn create_post(
    req: &HttpRequest,
    pool: &DbPool,
    tenant_slug: &str,
    board_slug: &str,
    title: &str,
    body: &str,
    attachments: &[String],
    user_id: Uuid,
) -> Result<PostCreatedDto, AppError> {
    let board = ensure_board_access(req, pool, tenant_slug, board_slug).await?;
    memberships::check_membership(pool, board.tenant_id, user_id)
        .await
        .map_err(|_| AppError::Forbidden)?;

    let created = post_repository::create_post(pool, board.tenant_id, board.board_id, user_id, title, body, attachments)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %board.tenant_id, board_slug = board_slug, user_id = %user_id, "error creating post");
            AppError::InternalServerError
        })?;

    // The author follows their own post so activity on it reaches them.
    let _ = crate::repositories::subscription_repository::ensure_follow(pool, created.id, user_id).await;

    Ok(created)
}

pub async fn update_post(
    pool: &DbPool,
    post_id: Uuid,
    body: &UpdatePostRequest,
    user_id: Uuid,
) -> Result<(), AppError> {
    let post = post_repository::get_editable_post(pool, post_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post for update");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    if post.is_hidden || post.deleted_at.is_some() {
        return Err(AppError::NotFound);
    }

    if post.user_id != user_id {
        return Err(AppError::Forbidden);
    }

    post_repository::update_post_content(pool, post_id, body.title.trim(), body.body.trim())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error updating post");
            AppError::InternalServerError
        })
}

pub async fn ensure_post_interaction_access(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
    require_unlocked: bool,
) -> Result<PostInteractionAccess, AppError> {
    let post = post_repository::find_post_access(pool, post_id, None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, post_id = %post_id, user_id = %user_id, "error fetching post interaction access");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    if post.is_hidden || post.deleted_at.is_some() {
        return Err(AppError::NotFound);
    }

    if require_unlocked && post.is_locked {
        return Err(AppError::Forbidden);
    }

    memberships::check_membership(pool, post.tenant_id, user_id)
        .await
        .map_err(|_| AppError::Forbidden)?;

    Ok(PostInteractionAccess {
        tenant_id: post.tenant_id,
    })
}
