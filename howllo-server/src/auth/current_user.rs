use crate::auth::hosted_rooiam;
use crate::auth::RooiamClient;
use crate::config::Settings;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::users::User;
use actix_web::dev::Payload;
use actix_web::{web, FromRequest, HttpRequest};
use futures::future::LocalBoxFuture;

pub struct AuthenticatedUser(pub User);

async fn authenticate_with_request(req: &HttpRequest) -> Result<Option<User>, AppError> {
    let pool = req
        .app_data::<web::Data<DbPool>>()
        .ok_or(AppError::InternalServerError)?;

    let auth_header = match req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        Some(token) => token,
        None => return Ok(None),
    };

    if auth_header.starts_with("howllo_") {
        return Ok(None);
    }

    let settings = req
        .app_data::<web::Data<Settings>>()
        .ok_or(AppError::InternalServerError)?;
    let client = RooiamClient::new(settings.rooiam_jwt_secret.clone());

    let (subject, email, name) = match client.validate_token(auth_header) {
        Ok(claims) => (claims.sub, claims.email, claims.name),
        Err(_) => {
            let userinfo = hosted_rooiam::fetch_userinfo(settings.get_ref(), auth_header)
                .await?
                .ok_or(AppError::Unauthorized)?;
            (userinfo.sub, userinfo.email, userinfo.name)
        }
    };

    let user = sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (rooiam_subject, email, display_name)
        VALUES ($1, $2, $3)
        ON CONFLICT (rooiam_subject) 
        DO UPDATE SET email = EXCLUDED.email, display_name = EXCLUDED.display_name
        RETURNING id, rooiam_subject, email, display_name, created_at, updated_at
        "#,
        subject,
        email.unwrap_or_else(|| "no-email@example.com".to_string()),
        name.unwrap_or_else(|| "Unknown User".to_string())
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "database error resolving authenticated user");
        AppError::InternalServerError
    })?;

    Ok(Some(user))
}

pub async fn maybe_authenticated_user(req: &HttpRequest) -> Result<Option<User>, AppError> {
    authenticate_with_request(req).await
}

impl FromRequest for AuthenticatedUser {
    type Error = AppError;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let req = req.clone();

        Box::pin(async move {
            match authenticate_with_request(&req).await? {
                Some(user) => Ok(AuthenticatedUser(user)),
                None => Err(AppError::Unauthorized),
            }
        })
    }
}
