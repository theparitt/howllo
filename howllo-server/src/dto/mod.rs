use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{errors::AppError, statuses::FeedbackStatus};

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct BoardListItemDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub board_type: String,
    pub intro_text: Option<String>,
    pub allow_votes: bool,
    pub allow_comments: bool,
    pub icon_url: Option<String>,
    pub background_color: Option<String>,
    pub dashboard_sections: Vec<String>,
    pub is_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct BoardDetailDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub board_type: String,
    pub intro_text: Option<String>,
    pub allow_votes: bool,
    pub allow_comments: bool,
    pub is_private: bool,
    pub is_enabled: bool,
    pub first_enabled_at: Option<DateTime<Utc>>,
    pub icon_url: Option<String>,
    pub background_color: Option<String>,
    pub dashboard_sections: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBoardRequest {
    pub tenant_slug: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub board_type: String,
    #[serde(default)]
    pub intro_text: Option<String>,
    #[serde(default)]
    pub allow_votes: Option<bool>,
    #[serde(default)]
    pub allow_comments: Option<bool>,
    pub is_private: bool,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub dashboard_sections: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBoardRequest {
    pub name: String,
    pub description: Option<String>,
    pub board_type: String,
    #[serde(default)]
    pub intro_text: Option<String>,
    #[serde(default)]
    pub allow_votes: Option<bool>,
    #[serde(default)]
    pub allow_comments: Option<bool>,
    pub is_private: bool,
    #[serde(default)]
    pub is_enabled: Option<bool>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub dashboard_sections: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PostListItemDto {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub vote_count: i32,
    pub comment_count: i64,
    pub duplicate_of_post_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub is_hidden: bool,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct TagSummaryItemDto {
    pub tag_id: Uuid,
    pub tag_slug: String,
    pub tag_name: String,
    pub usage_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
    pub has_next: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditLogItemDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_user_id: Uuid,
    pub actor_display_name: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub action: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub reason: Option<String>,
    pub request_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OfficialResponseSummaryDto {
    pub id: Uuid,
    pub body: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostDetailDto {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub status: String,
    pub author_display_name: String,
    pub vote_count: i32,
    pub comment_count: i64,
    pub is_locked: bool,
    pub duplicate_of_post_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub board_slug: String,
    pub tags: Vec<TagDto>,
    pub follow_state: Option<PostFollowStateDto>,
    pub official_response: Option<OfficialResponseSummaryDto>,
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub tenant_slug: String,
    pub title: String,
    pub body: String,
    /// Public URLs of images attached to this post (from /api/uploads). Optional.
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostRequest {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct PostCreatedDto {
    pub id: Uuid,
    pub review_state: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct CommentCreatedDto {
    pub id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommentListItemDto {
    pub id: Uuid,
    pub body: String,
    pub is_official_response: bool,
    pub comment_type: String,
    pub created_at: DateTime<Utc>,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostStatusRequest {
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVisibilityRequest {
    pub is_hidden: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCommentVisibilityRequest {
    pub is_hidden: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLockRequest {
    pub is_locked: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOfficialCommentRequest {
    pub is_official: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostDuplicateRequest {
    pub duplicate_of_post_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostFollowStateDto {
    pub is_following: bool,
    pub notify_on_status_change: bool,
    pub notify_on_official_response: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MembershipItemDto {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMemberRequest {
    pub tenant_slug: String,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRoleRequest {
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusHistoryItemDto {
    pub id: Uuid,
    pub old_status: Option<String>,
    pub new_status: String,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub actor_display_name: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct TagDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTagRequest {
    pub tenant_slug: String,
    pub slug: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTagRequest {
    pub name: String,
    pub color: Option<String>,
}

impl CreatePostRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.tenant_slug.trim().is_empty() {
            return Err(AppError::Validation("tenant_slug is required".to_string()));
        }
        if self.title.trim().is_empty() {
            return Err(AppError::Validation("title is required".to_string()));
        }
        if self.title.trim().chars().count() > 255 {
            return Err(AppError::Validation(
                "title must be 255 characters or fewer".to_string(),
            ));
        }
        if self.body.trim().is_empty() {
            return Err(AppError::Validation("body is required".to_string()));
        }

        Ok(())
    }
}

impl CreateBoardRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.tenant_slug.trim().is_empty() {
            return Err(AppError::Validation("tenant_slug is required".to_string()));
        }
        if self.slug.trim().is_empty() {
            return Err(AppError::Validation("slug is required".to_string()));
        }
        if self.name.trim().is_empty() {
            return Err(AppError::Validation("name is required".to_string()));
        }
        if self.board_type.trim().is_empty() {
            return Err(AppError::Validation("board_type is required".to_string()));
        }
        if self.intro_text.as_ref().is_some_and(|text| text.chars().count() > 240) {
            return Err(AppError::Validation("intro_text must be 240 characters or fewer".to_string()));
        }
        Ok(())
    }
}

impl UpdateBoardRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.name.trim().is_empty() {
            return Err(AppError::Validation("name is required".to_string()));
        }
        if self.board_type.trim().is_empty() {
            return Err(AppError::Validation("board_type is required".to_string()));
        }
        if self.intro_text.as_ref().is_some_and(|text| text.chars().count() > 240) {
            return Err(AppError::Validation("intro_text must be 240 characters or fewer".to_string()));
        }
        Ok(())
    }
}

impl CreateCommentRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.body.trim().is_empty() {
            return Err(AppError::Validation("body is required".to_string()));
        }

        Ok(())
    }
}

impl UpdatePostRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.title.trim().is_empty() {
            return Err(AppError::Validation("title is required".to_string()));
        }
        if self.title.trim().chars().count() > 255 {
            return Err(AppError::Validation(
                "title must be 255 characters or fewer".to_string(),
            ));
        }
        if self.body.trim().is_empty() {
            return Err(AppError::Validation("body is required".to_string()));
        }
        Ok(())
    }
}

impl UpdatePostStatusRequest {
    pub fn parse_status(&self) -> Result<FeedbackStatus, AppError> {
        FeedbackStatus::parse(&self.status)
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if let Some(reason) = &self.reason {
            if reason.chars().count() > 2000 {
                return Err(AppError::Validation(
                    "reason must be 2000 characters or fewer".to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl CreateTagRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.tenant_slug.trim().is_empty() {
            return Err(AppError::Validation("tenant_slug is required".to_string()));
        }
        if self.slug.trim().is_empty() {
            return Err(AppError::Validation("slug is required".to_string()));
        }
        if self.name.trim().is_empty() {
            return Err(AppError::Validation("name is required".to_string()));
        }
        Ok(())
    }
}

impl UpdateTagRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.name.trim().is_empty() {
            return Err(AppError::Validation("name is required".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ModerationQueueItemDto {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub board_slug: String,
    pub status: String,
    pub is_hidden: bool,
    pub review_state: String,
    pub review_reason: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub vote_count: i32,
    pub comment_count: i64,
    pub duplicate_of_post_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub author_display_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostActivityDto {
    pub status_history: Vec<StatusHistoryItemDto>,
    pub official_response: Option<OfficialResponseSummaryDto>,
    pub duplicate_of_post_id: Option<Uuid>,
    pub follower_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BoardSummaryDto {
    pub board_id: Uuid,
    pub board_slug: String,
    pub board_name: String,
    pub total_posts: i64,
    pub posts_by_status: HashMap<String, i64>,
    pub total_votes: i64,
    pub total_comments: i64,
    pub delete_posts_count: i64,
    pub delete_comments_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoadmapGroupedDto {
    pub planned: Vec<PostListItemDto>,
    pub in_progress: Vec<PostListItemDto>,
    pub done: Vec<PostListItemDto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedCommentsDto {
    pub items: Vec<CommentListItemDto>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
    pub has_next: bool,
}

#[cfg(test)]
mod tests {
    use super::{CreateCommentRequest, CreatePostRequest, UpdatePostStatusRequest};
    use crate::statuses::FeedbackStatus;

    #[test]
    fn rejects_empty_post_fields() {
        let request = CreatePostRequest {
            tenant_slug: "".to_string(),
            title: " ".to_string(),
            body: "".to_string(),
            attachments: Vec::new(),
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn rejects_empty_comment_body() {
        let request = CreateCommentRequest {
            body: " ".to_string(),
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn parses_status_request() {
        let request = UpdatePostStatusRequest {
            status: "planned".to_string(),
            reason: None,
        };

        assert_eq!(request.parse_status().unwrap(), FeedbackStatus::Planned);
    }
}
