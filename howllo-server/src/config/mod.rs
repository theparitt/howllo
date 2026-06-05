use std::env;

#[derive(Debug, Clone)]
pub struct Settings {
    pub database_url: String,
    pub bind_address: String,
    pub rooiam_jwt_secret: String,
    pub rooiam_hosted_userinfo_url: Option<String>,
    /// One-time key gating local admin setup (first password). Until the admin
    /// is bootstrapped, the setup endpoint requires this value. Set via
    /// HOWLLO_ADMIN_BOOTSTRAP_KEY. If unset, local admin setup is disabled.
    pub admin_bootstrap_key: Option<String>,
    pub allowed_origins: Vec<String>,
    pub rate_limit_enabled: bool,
    pub public_write_rate_limit: u32,
    pub max_post_body_chars: usize,
    pub max_comment_body_chars: usize,
    pub webhook_timeout_ms: u64,
    pub ai: AiSettings,
}

#[derive(Debug, Clone)]
pub struct AiSettings {
    pub enabled: bool,
    pub provider: AiProvider,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiProvider {
    Disabled,
    Local,
}

impl Settings {
    pub fn from_env() -> Self {
        Self {
            // App reads HOWLLO_DATABASE_URL. We fall back to DATABASE_URL because
            // the sqlx CLI and compile-time query macros require that exact name;
            // keeping the fallback lets one value serve both when convenient.
            database_url: env::var("HOWLLO_DATABASE_URL")
                .or_else(|_| env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgres://localhost/howllo".to_string()),
            bind_address: env::var("HOWLLO_BIND_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:5110".to_string()),
            rooiam_jwt_secret: env::var("HOWLLO_JWT_SECRET")
                .unwrap_or_else(|_| "dev-secret".to_string()),
            rooiam_hosted_userinfo_url: env::var("HOWLLO_ROOIAM_HOSTED_USERINFO_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            admin_bootstrap_key: env::var("HOWLLO_ADMIN_BOOTSTRAP_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            allowed_origins: env::var("HOWLLO_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3000,http://127.0.0.1:3000".to_string())
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect(),
            rate_limit_enabled: bool_flag("HOWLLO_RATE_LIMIT_ENABLED", false),
            public_write_rate_limit: env::var("HOWLLO_PUBLIC_WRITE_RATE_LIMIT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(60),
            max_post_body_chars: env::var("HOWLLO_MAX_POST_BODY_CHARS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(10_000),
            max_comment_body_chars: env::var("HOWLLO_MAX_COMMENT_BODY_CHARS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(4_000),
            webhook_timeout_ms: env::var("HOWLLO_WEBHOOK_TIMEOUT_MS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(5_000),
            ai: AiSettings::from_env(),
        }
    }
}

impl AiSettings {
    fn from_env() -> Self {
        let enabled = bool_flag("HOWLLO_AI_ENABLED", false);

        let provider = if enabled {
            AiProvider::Local
        } else {
            AiProvider::Disabled
        };

        Self {
            enabled,
            provider,
            base_url: env::var("HOWLLO_AI_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string()),
            model: env::var("HOWLLO_AI_MODEL").unwrap_or_else(|_| "gemma4".to_string()),
        }
    }
}

fn bool_flag(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::{AiProvider, Settings};

    #[test]
    fn settings_have_local_defaults() {
        let settings = Settings::from_env();

        assert!(!settings.database_url.is_empty());
        assert!(settings.bind_address.contains(':'));
        assert!(!settings.rooiam_jwt_secret.is_empty());
        assert!(!settings.allowed_origins.is_empty());
        assert_eq!(settings.ai.provider, AiProvider::Disabled);
        assert_eq!(settings.ai.model, "gemma4");
    }
}
