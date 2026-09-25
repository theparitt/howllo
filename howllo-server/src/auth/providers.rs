//! Configured authentication providers. Provider secrets stay server-side.
use crate::errors::AppError;
use actix_web::{get, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
pub struct OidcProviderConfig {
    pub id: String,
    pub display_name: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    #[serde(default)]
    pub token_endpoint_auth_method: Option<String>,
    #[serde(default = "default_scopes")]
    pub scopes: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl std::fmt::Debug for OidcProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcProviderConfig")
            .field("id", &self.id)
            .field("display_name", &self.display_name)
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "token_endpoint_auth_method",
                &self.token_endpoint_auth_method,
            )
            .field("scopes", &self.scopes)
            .field("enabled", &self.enabled)
            .finish()
    }
}

fn default_scopes() -> String {
    "openid profile email".into()
}
fn default_enabled() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct PublicProvider {
    pub id: String,
    pub display_name: String,
    pub kind: &'static str,
    pub login_url: String,
}

pub fn configured_oidc() -> Result<Vec<OidcProviderConfig>, AppError> {
    let raw = std::env::var("HOWLLO_OIDC_PROVIDERS").unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(vec![]);
    }
    let providers: Vec<OidcProviderConfig> = serde_json::from_str(&raw).map_err(|error| {
        tracing::error!(error = %error, "invalid OIDC provider configuration");
        AppError::InternalServerError
    })?;
    let mut seen = std::collections::HashSet::new();
    for provider in &providers {
        if provider.id.is_empty()
            || matches!(
                provider.id.as_str(),
                "local" | "local-admin" | "workspace-sso" | "invited"
            )
            || !provider
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            || !seen.insert(&provider.id)
            || provider.client_id.is_empty()
            || provider
                .token_endpoint_auth_method
                .as_deref()
                .is_some_and(|method| {
                    !matches!(
                        method,
                        "client_secret_basic" | "client_secret_post" | "none"
                    )
                })
            || provider
                .token_endpoint_auth_method
                .as_deref()
                .is_some_and(|method| {
                    method != "none" && provider.client_secret.as_deref().unwrap_or("").is_empty()
                })
            || provider.display_name.trim().is_empty()
            || !provider
                .scopes
                .split_whitespace()
                .any(|scope| scope == "openid")
        {
            return Err(AppError::InternalServerError);
        }
        let url = url::Url::parse(&provider.issuer).map_err(|_| AppError::InternalServerError)?;
        if url.scheme() != "https"
            && !(url.scheme() == "http"
                && matches!(url.host_str(), Some("localhost" | "127.0.0.1")))
        {
            return Err(AppError::InternalServerError);
        }
    }
    Ok(providers)
}

pub fn oidc_provider(id: &str) -> Result<OidcProviderConfig, AppError> {
    configured_oidc()?
        .into_iter()
        .find(|item| item.id == id && item.enabled)
        .ok_or(AppError::NotFound)
}

pub fn local_enabled() -> bool {
    std::env::var("HOWLLO_AUTH_LOCAL_ENABLED").unwrap_or_else(|_| "true".into()) != "false"
}

#[get("/api/auth/providers")]
pub async fn list_providers() -> Result<impl Responder, AppError> {
    let mut providers = Vec::new();
    if local_enabled() {
        providers.push(PublicProvider {
            id: "local".into(),
            display_name: "Local account".into(),
            kind: "local",
            login_url: "/api/auth/local/login".into(),
        });
    }
    for provider in configured_oidc()?.into_iter().filter(|p| p.enabled) {
        providers.push(PublicProvider {
            login_url: format!("/api/auth/login/{}", provider.id),
            id: provider.id,
            display_name: provider.display_name,
            kind: "oidc",
        });
    }
    Ok(HttpResponse::Ok().json(providers))
}

#[cfg(test)]
mod tests {
    use super::OidcProviderConfig;
    #[test]
    fn secrets_are_not_serializable() {
        let provider: OidcProviderConfig = serde_json::from_str(r#"{"id":"company","display_name":"Company","issuer":"https://example.com","client_id":"id","client_secret":"secret"}"#).unwrap();
        assert!(provider.enabled);
        assert!(provider.scopes.contains("openid"));
    }
}
