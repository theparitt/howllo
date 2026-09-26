use actix_web::HttpRequest;
use chrono::{DateTime, Duration, Utc};
use sqlx::Row;
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
use crate::policy;
use crate::repositories::{post_repository, tenant_branding_repository};
use crate::services::spam_guard;
use crate::statuses::FeedbackStatus;

pub struct BoardPostListInput<'a> {
    pub req: &'a HttpRequest,
    pub pool: &'a DbPool,
    pub board_slug: &'a str,
    pub tenant_slug: &'a str,
    pub sort: &'a str,
    pub status: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub category: Option<&'a str>,
    pub q: Option<&'a str>,
    pub page: i64,
    pub per_page: i64,
    pub include_hidden: bool,
}

pub struct PostInteractionAccess {
    pub tenant_id: Uuid,
    pub board_id: Uuid,
    pub allow_votes: bool,
    pub allow_comments: bool,
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
        memberships::require_private_access(pool, board.tenant_id, user.id).await?;
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
        memberships::require_private_access(pool, post.tenant_id, user.id).await?;
    }

    Ok(post)
}

pub async fn list_board_posts(
    input: BoardPostListInput<'_>,
) -> Result<PaginatedResponse<PostListItemDto>, AppError> {
    if input.q.is_some_and(|q| q.len() > 100) {
        return Err(AppError::Validation(
            "Search is limited to 100 characters".into(),
        ));
    }
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
        input.category,
        input.q,
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
            category: input.category,
            q: input.q,
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
        category_name: detail.category_name,
        category_color: detail.category_color,
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
        is_pinned: detail.is_pinned,
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
    category_id: Option<Uuid>,
    tag_ids: &[Uuid],
    user_id: Uuid,
) -> Result<PostCreatedDto, AppError> {
    let board = ensure_board_access(req, pool, tenant_slug, board_slug).await?;
    if board.is_private && !attachments.is_empty() {
        return Err(AppError::Validation(
            "Screenshots are unavailable on private boards until private storage is configured."
                .into(),
        ));
    }
    if tag_ids.len() > 3
        || tag_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != tag_ids.len()
    {
        return Err(AppError::Validation("Choose up to 3 different tags".into()));
    }
    if !tag_ids.is_empty() {
        let valid_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM tags WHERE tenant_id=$1 AND id=ANY($2)")
                .bind(board.tenant_id)
                .bind(tag_ids)
                .fetch_one(pool)
                .await
                .map_err(|_| AppError::InternalServerError)?;
        if valid_count != tag_ids.len() as i64 {
            return Err(AppError::Validation(
                "Choose tags from this workspace".into(),
            ));
        }
    }
    if let Some(category_id) = category_id {
        let belongs: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM board_categories WHERE id=$1 AND board_id=$2)",
        )
        .bind(category_id)
        .bind(board.board_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;
        if !belongs {
            return Err(AppError::Validation(
                "Choose a category from this board".into(),
            ));
        }
    }
    if matches!(
        board.board_type.as_str(),
        "announcements" | "changelog" | "updates"
    ) {
        require_permission(pool, board.tenant_id, user_id, Permission::ModerateContent).await?;
    }
    policy::enforce_ip_policy(req, pool, board.tenant_id).await?;
    memberships::check_membership(pool, board.tenant_id, user_id)
        .await
        .map_err(|_| AppError::Forbidden)?;

    let spam_settings = tenant_branding_repository::get_by_tenant_slug(pool, tenant_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_slug, "error fetching post approval setting");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;
    let approval_required = spam_settings.require_post_approval;
    let limits = policy::workspace_policy(pool, board.tenant_id)
        .await?
        .effective;

    // Serialize writes by account and workspace so concurrent requests cannot
    // bypass the database-backed posting limits.
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError)?;
    let lock_key = format!("post:{}:{}", board.tenant_id, user_id);
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(&lock_key)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;
    let activity = sqlx::query(
        r#"SELECT
            count(*) FILTER (WHERE created_at > now() - interval '1 hour')::bigint AS last_hour,
            count(*) FILTER (WHERE created_at > now() - interval '1 day')::bigint AS last_day,
            max(created_at) AS latest
        FROM posts WHERE tenant_id = $1 AND user_id = $2"#,
    )
    .bind(board.tenant_id)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError)?;
    let last_hour: i64 = activity.get("last_hour");
    let last_day: i64 = activity.get("last_day");
    let latest: Option<DateTime<Utc>> = activity.get("latest");
    if last_hour >= i64::from(limits.posts_per_hour) || last_day >= i64::from(limits.posts_per_day)
    {
        spam_guard::flag_account(&mut tx, board.tenant_id, user_id, "Posting rate exceeded")
            .await?;
        tx.commit()
            .await
            .map_err(|_| AppError::InternalServerError)?;
        return Err(AppError::TooManyRequests(
            "You have reached the posting limit. Please try again later.".to_string(),
        ));
    }
    if latest.is_some_and(|at| at > Utc::now() - Duration::seconds(10)) {
        return Err(AppError::TooManyRequests(
            "Please wait before posting again.".to_string(),
        ));
    }
    if spam_guard::board_is_paused(&mut tx, board.board_id, "post", limits.board_posts_per_10m)
        .await?
    {
        tx.commit()
            .await
            .map_err(|_| AppError::InternalServerError)?;
        return Err(AppError::TooManyRequests(
            "This board is receiving unusually high activity. Posting is paused for a few minutes; please try again shortly.".to_string(),
        ));
    }

    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM posts WHERE tenant_id = $1 AND lower(title) = lower($2) AND created_at > now() - interval '1 day')",
    )
    .bind(board.tenant_id)
    .bind(title.trim())
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError)?;
    let links = body.matches("https://").count() + body.matches("http://").count();
    let flagged = spam_guard::account_is_flagged(&mut tx, board.tenant_id, user_id).await?;
    let review_reason = if approval_required {
        Some("Workspace approval required")
    } else if flagged {
        Some("Account exceeded posting limit")
    } else if duplicate {
        Some("Similar post title")
    } else if links >= 2 {
        Some("Multiple links")
    } else if last_day == 0 && links > 0 {
        Some("New member posted a link")
    } else {
        None
    };
    let review_state = if review_reason.is_some() {
        "pending"
    } else {
        "approved"
    };

    let created = post_repository::create_post(&mut tx, board.tenant_id, board.board_id, user_id, title, body, attachments, category_id, review_state, review_reason)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %board.tenant_id, board_slug = board_slug, user_id = %user_id, "error creating post");
            AppError::InternalServerError
        })?;
    for tag_id in tag_ids {
        crate::repositories::tag_repository::attach_tag_to_post(&mut tx, created.id, *tag_id)
            .await
            .map_err(|_| AppError::InternalServerError)?;
    }
    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // The author follows their own post so activity on it reaches them.
    let _ = crate::repositories::subscription_repository::ensure_follow(pool, created.id, user_id)
        .await;

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

    memberships::check_membership(pool, post.tenant_id, user_id)
        .await
        .map_err(|_| AppError::Forbidden)?;

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
        board_id: post.board_id,
        allow_votes: post.allow_votes,
        allow_comments: post.allow_comments,
    })
}
