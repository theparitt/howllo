use actix_web::HttpRequest;

use crate::auth::maybe_authenticated_user;
use crate::db::DbPool;
use crate::dto::PostListItemDto;
use crate::errors::AppError;
use crate::repositories::roadmap_repository;
use crate::statuses::FeedbackStatus;

pub async fn get_roadmap_items(
    req: &HttpRequest,
    pool: &DbPool,
    tenant_slug: &str,
    status_filter: Option<&str>,
    tag_filter: Option<&str>,
    board_slug_filter: Option<&str>,
) -> Result<Vec<PostListItemDto>, AppError> {
    let user_id = maybe_authenticated_user(req).await?.map(|user| user.id);

    let status_filter = match status_filter {
        Some(status) => {
            let parsed = FeedbackStatus::parse(status)?;
            let db_status = parsed.as_db_str();
            if !matches!(db_status, "planned" | "in_progress" | "done") {
                return Err(AppError::Validation(
                    "roadmap status must be planned, in_progress, or done".to_string(),
                ));
            }
            Some(db_status)
        }
        None => None,
    };

    roadmap_repository::fetch_roadmap_items(pool, tenant_slug, user_id, status_filter, tag_filter, board_slug_filter)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_slug = tenant_slug, status_filter = ?status_filter, tag_filter = ?tag_filter, "error fetching roadmap items");
            AppError::InternalServerError
        })
}
