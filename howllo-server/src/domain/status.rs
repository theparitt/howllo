use serde::{Deserialize, Serialize};

use crate::errors::AppError;

pub const ALL_FEEDBACK_STATUSES: [&str; 5] =
    ["under_review", "planned", "in_progress", "done", "declined"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackStatus {
    UnderReview,
    Planned,
    InProgress,
    Done,
    Declined,
}

impl FeedbackStatus {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value.trim() {
            "under_review" => Ok(Self::UnderReview),
            "planned" => Ok(Self::Planned),
            "in_progress" => Ok(Self::InProgress),
            "done" => Ok(Self::Done),
            "declined" => Ok(Self::Declined),
            other => Err(AppError::BadRequest(format!("invalid status: {other}"))),
        }
    }

    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::UnderReview => "under_review",
            Self::Planned => "planned",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Declined => "declined",
        }
    }
}

pub fn allowed_transitions(current: FeedbackStatus) -> Vec<FeedbackStatus> {
    use FeedbackStatus::*;
    match current {
        UnderReview => vec![Planned, Declined],
        Planned => vec![InProgress, Declined],
        InProgress => vec![Done, Planned],
        Done => vec![InProgress],
        Declined => vec![UnderReview],
    }
}

#[cfg(test)]
mod tests {
    use super::FeedbackStatus;

    #[test]
    fn parses_supported_status_variants() {
        assert_eq!(
            FeedbackStatus::parse("under_review").unwrap(),
            FeedbackStatus::UnderReview
        );
        assert_eq!(
            FeedbackStatus::parse("in_progress").unwrap(),
            FeedbackStatus::InProgress
        );
        assert_eq!(FeedbackStatus::parse("done").unwrap(), FeedbackStatus::Done);
    }

    #[test]
    fn rejects_legacy_hyphenated_statuses() {
        assert!(FeedbackStatus::parse("under-review").is_err());
        assert!(FeedbackStatus::parse("in-progress").is_err());
    }
}
