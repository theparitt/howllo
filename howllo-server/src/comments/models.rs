use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentType {
    User,
    Official,
    Moderator,
}

impl CommentType {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "user" => Some(Self::User),
            "official" => Some(Self::Official),
            "moderator" => Some(Self::Moderator),
            _ => None,
        }
    }

    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Official => "official",
            Self::Moderator => "moderator",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: Uuid,
    pub post_id: Uuid,
    pub user_id: Uuid,
    pub body: String,
    pub is_official_response: bool,
    pub comment_type: String,
    pub is_hidden: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::CommentType;

    #[test]
    fn parses_comment_type_values() {
        assert_eq!(CommentType::parse("user"), Some(CommentType::User));
        assert_eq!(CommentType::parse("official"), Some(CommentType::Official));
        assert_eq!(
            CommentType::parse("moderator"),
            Some(CommentType::Moderator)
        );
        assert_eq!(CommentType::parse("invalid"), None);
    }
}
