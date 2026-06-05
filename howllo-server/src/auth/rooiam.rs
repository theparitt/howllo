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
