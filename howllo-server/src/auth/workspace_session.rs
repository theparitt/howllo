use actix_web::{delete, post, web, HttpRequest, HttpResponse, Responder};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::rooiam::{resolve_rooiam_access_token, ResolvedRooiamIdentity};
use crate::config::Settings;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::users::User;

const WORKSPACE_SESSION_PREFIX: &str = "howllo_ws_";

#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceSessionRequest {
    pub tenant_slug: String,
}

#[derive(Debug, Serialize)]
pub struct CreateWorkspaceSessionResponse {
    pub session_token: String,
}

#[derive(Debug, Clone)]
pub struct WorkspaceSessionAuth {
    pub tenant_id: Uuid,
    pub user: User,
}

pub fn is_workspace_session_token(token: &str) -> bool {
    token.starts_with(WORKSPACE_SESSION_PREFIX)
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn raw_bearer_token(req: &HttpRequest) -> Option<&str> {
    req.headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
}

async fn upsert_rooiam_user(
    pool: &DbPool,
    identity: ResolvedRooiamIdentity,
) -> Result<User, AppError> {
    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (rooiam_subject, email, display_name)
        VALUES ($1, $2, $3)
        ON CONFLICT (rooiam_subject)
        DO UPDATE SET
            email = EXCLUDED.email,
            updated_at = NOW()
        RETURNING id, rooiam_subject, email, display_name, avatar_url, created_at, updated_at
        "#,
    )
    .bind(identity.sub)
    .bind(
        identity
            .email
            .unwrap_or_else(|| "no-email@example.com".to_string()),
    )
    .bind(identity.name.unwrap_or_else(|| "Unknown User".to_string()))
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "database error resolving workspace-session user");
        AppError::InternalServerError
    })
}

pub async fn resolve_workspace_session_from_token(
    pool: &DbPool,
    token: &str,
) -> Result<Option<WorkspaceSessionAuth>, AppError> {
    if !is_workspace_session_token(token) {
        return Ok(None);
    }

    let token_hash = hash_token(token);
    let row = sqlx::query_as::<_, User>(
        r#"
        SELECT
            u.id,
            u.rooiam_subject,
            u.email,
            u.display_name,
            u.avatar_url,
            u.created_at,
            u.updated_at
        FROM workspace_sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.token_hash = $1
          AND s.revoked_at IS NULL
          AND s.expires_at > NOW()
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "error resolving workspace session user");
        AppError::InternalServerError
    })?;

    let tenant_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT tenant_id
        FROM workspace_sessions
        WHERE token_hash = $1
          AND revoked_at IS NULL
          AND expires_at > NOW()
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "error resolving workspace session tenant");
        AppError::InternalServerError
    })?;

    let Some(user) = row else {
        return Ok(None);
    };
    let Some(tenant_id) = tenant_id else {
        return Ok(None);
    };

    let _ = sqlx::query("UPDATE workspace_sessions SET last_used_at = NOW() WHERE token_hash = $1")
        .bind(&token_hash)
        .execute(pool)
        .await;

    Ok(Some(WorkspaceSessionAuth { tenant_id, user }))
}

pub async fn resolve_workspace_session_from_request(
    req: &HttpRequest,
) -> Result<Option<WorkspaceSessionAuth>, AppError> {
    let Some(token) = raw_bearer_token(req) else {
        return Ok(None);
    };

    let pool = req
        .app_data::<web::Data<DbPool>>()
        .ok_or(AppError::InternalServerError)?;

    resolve_workspace_session_from_token(pool.get_ref(), token).await
}

#[post("/api/auth/workspace-session")]
pub async fn create_workspace_session(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    body: web::Json<CreateWorkspaceSessionRequest>,
) -> Result<impl Responder, AppError> {
    let access_token = raw_bearer_token(&req).ok_or(AppError::Unauthorized)?;
    if access_token.starts_with("howllo_") {
        return Err(AppError::Unauthorized);
    }

    let tenant_slug = body.tenant_slug.trim();
    if tenant_slug.is_empty() {
        return Err(AppError::Validation("tenant_slug is required".to_string()));
    }

    let identity = resolve_rooiam_access_token(settings.get_ref(), access_token).await?;
    let user = upsert_rooiam_user(pool.get_ref(), identity).await?;

    let tenant_id =
        crate::repositories::membership_repository::resolve_tenant_id(pool.get_ref(), tenant_slug)
            .await?;

    // Signing into a workspace makes the user a `member` of that tenant so they
    // can participate on public boards (create posts, vote, comment). Use
    // DO NOTHING so we never downgrade an existing owner/admin/moderator.
    sqlx::query(
        r#"
        INSERT INTO memberships (tenant_id, user_id, role)
        VALUES ($1, $2, 'member')
        ON CONFLICT (tenant_id, user_id) DO NOTHING
        "#,
    )
    .bind(tenant_id)
    .bind(user.id)
    .execute(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_slug, user_id = %user.id, "error ensuring workspace membership");
        AppError::InternalServerError
    })?;

    let random = format!(
        "{}{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
        &Uuid::new_v4().simple().to_string()[..8]
    );
    let session_token = format!("{WORKSPACE_SESSION_PREFIX}{random}");
    let token_hash = hash_token(&session_token);
    let expires_at = Utc::now() + Duration::days(30);

    sqlx::query(
        r#"
        INSERT INTO workspace_sessions (tenant_id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(tenant_id)
    .bind(user.id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_slug, user_id = %user.id, "error creating workspace session");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Created().json(CreateWorkspaceSessionResponse { session_token }))
}

#[delete("/api/auth/workspace-session")]
pub async fn revoke_workspace_session(
    req: HttpRequest,
    pool: web::Data<DbPool>,
) -> Result<impl Responder, AppError> {
    let token = raw_bearer_token(&req).ok_or(AppError::Unauthorized)?;
    if !is_workspace_session_token(token) {
        return Err(AppError::Unauthorized);
    }

    sqlx::query("UPDATE workspace_sessions SET revoked_at = NOW() WHERE token_hash = $1")
        .bind(hash_token(token))
        .execute(pool.get_ref())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "error revoking workspace session");
            AppError::InternalServerError
        })?;

    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;

    use crate::db;
    use crate::http::test_support::{
        bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
    };
    use crate::startup;

    // Signing into a workspace must make the user a member so they can
    // immediately participate on public boards. This is the exact path a new
    // end-user takes; before the membership grant it returned 403 Forbidden.
    #[actix_web::test]
    async fn workspace_session_signin_grants_member_and_allows_posting() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(crate::realtime::Hub::new()))
                .configure(startup::configure),
        )
        .await;

        // A brand-new identity that is NOT yet a member of the tenant.
        let subject = format!("newcomer-{}", uuid::Uuid::new_v4().simple());
        let token = bearer_for(
            &subject,
            "newcomer@example.com",
            "New Comer",
            &settings.rooiam_jwt_secret,
        );

        // Sign into the workspace -> should mint a session token.
        let session_request = test::TestRequest::post()
            .uri("/api/auth/workspace-session")
            .insert_header(("Authorization", token))
            .set_json(json!({ "tenant_slug": seed.tenant_slug }))
            .to_request();
        let session_response = test::call_service(&app, session_request).await;
        assert_eq!(session_response.status(), StatusCode::CREATED);
        let session_json = read_json(session_response).await;
        let session_token = session_json
            .get("session_token")
            .and_then(|v| v.as_str())
            .expect("session token")
            .to_string();

        // The same user can now create a post on a public board (was 403).
        let create_request = test::TestRequest::post()
            .uri(&format!("/api/boards/{}/posts", seed.board_slug))
            .insert_header(("Authorization", format!("Bearer {session_token}")))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "title": "My first request",
                "body": "Please add this feature."
            }))
            .to_request();
        let create_response = test::call_service(&app, create_request).await;
        assert_eq!(create_response.status(), StatusCode::CREATED);
    }

    // Signing in must never downgrade an existing owner/admin/moderator to member.
    #[actix_web::test]
    async fn workspace_session_signin_does_not_downgrade_existing_admin() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(crate::realtime::Hub::new()))
                .configure(startup::configure),
        )
        .await;

        // The seeded admin (role = "admin") signs into the workspace.
        let token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let session_request = test::TestRequest::post()
            .uri("/api/auth/workspace-session")
            .insert_header(("Authorization", token))
            .set_json(json!({ "tenant_slug": seed.tenant_slug }))
            .to_request();
        let session_response = test::call_service(&app, session_request).await;
        assert_eq!(session_response.status(), StatusCode::CREATED);

        // Role stays "admin" — the member grant used ON CONFLICT DO NOTHING.
        let role: String = sqlx::query_scalar(
            "SELECT role FROM memberships WHERE user_id = $1",
        )
        .bind(seed.admin_user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(role, "admin");
    }
}
