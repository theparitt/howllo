use crate::errors::AppError;

pub const ALL_MEMBERSHIP_ROLES: [&str; 4] = ["owner", "admin", "moderator", "member"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Admin,
    Moderator,
    Member,
}

impl Role {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value.trim() {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "moderator" => Ok(Self::Moderator),
            "member" => Ok(Self::Member),
            other => Err(AppError::BadRequest(format!("invalid role: {other}"))),
        }
    }

    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Moderator => "moderator",
            Self::Member => "member",
        }
    }

    pub fn is_admin_like(self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Moderator)
    }
}

#[cfg(test)]
mod tests {
    use super::Role;

    #[test]
    fn parses_supported_roles() {
        assert_eq!(Role::parse("owner").unwrap(), Role::Owner);
        assert_eq!(Role::parse("admin").unwrap(), Role::Admin);
        assert_eq!(Role::parse("moderator").unwrap(), Role::Moderator);
        assert_eq!(Role::parse("member").unwrap(), Role::Member);
    }
}
