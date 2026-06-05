use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub board_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub body: String,
    pub status: String,
    pub vote_count: i32,
    pub is_locked: bool,
    pub is_hidden: bool,
    pub duplicate_of_post_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
