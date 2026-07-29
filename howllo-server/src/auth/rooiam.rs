use crate::auth::hosted_rooiam;
use crate::config::Settings;
use crate::errors::AppError;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RooiamClaims {
    pub sub: String, // Rooiam subject
    pub exp: usize,
    pub email: Option<String>,
    pub name: Option<String>,
}

pub struct RooiamClient {
    jwt_secret: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedRooiamIdentity {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

impl RooiamClient {
    pub fn new(secret: String) -> Self {
        Self { jwt_secret: secret }
    }

    pub fn validate_token(&self, token: &str) -> Result<RooiamClaims, AppError> {
        let validation = Validation::new(Algorithm::HS256);
        let token_data = decode::<RooiamClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_ref()),
            &validation,
        )
        .map_err(|e| {
            tracing::warn!(error = %e, "jwt validation failed");
            AppError::Unauthorized
        })?;

        Ok(token_data.claims)
    }
}

pub async fn resolve_rooiam_access_token(
    settings: &Settings,
    access_token: &str,
) -> Result<ResolvedRooiamIdentity, AppError> {
    let client = RooiamClient::new(settings.rooiam_jwt_secret.clone());

    match client.validate_token(access_token) {
        Ok(claims) => Ok(ResolvedRooiamIdentity {
            sub: claims.sub,
            email: claims.email,
            name: claims.name,
        }),
        Err(_) => {
            let userinfo = hosted_rooiam::fetch_userinfo(settings, access_token)
                .await?
                .ok_or(AppError::Unauthorized)?;
            Ok(ResolvedRooiamIdentity {
                sub: userinfo.sub,
                email: userinfo.email,
                name: userinfo.name,
            })
        }
    }
}
