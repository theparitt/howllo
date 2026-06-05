use serde::Deserialize;

use crate::config::Settings;
use crate::errors::AppError;

#[derive(Debug, Deserialize)]
pub struct HostedRooiamUserInfo {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

pub async fn fetch_userinfo(
    settings: &Settings,
    access_token: &str,
) -> Result<Option<HostedRooiamUserInfo>, AppError> {
    let Some(url) = settings.rooiam_hosted_userinfo_url.as_deref() else {
        return Ok(None);
    };

    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, url, "hosted rooiam userinfo request failed");
            AppError::Unauthorized
        })?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::Unauthorized);
    }

    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), url, "hosted rooiam userinfo returned non-success");
        return Err(AppError::Unauthorized);
    }

    response
        .json::<HostedRooiamUserInfo>()
        .await
        .map(Some)
        .map_err(|error| {
            tracing::warn!(error = %error, url, "hosted rooiam userinfo decode failed");
            AppError::Unauthorized
        })
}
