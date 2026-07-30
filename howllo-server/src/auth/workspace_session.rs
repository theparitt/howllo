use actix_web::{delete, post, web, HttpRequest, HttpResponse, Responder};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

use crate::auth::rooiam::{resolve_rooiam_access_token, ResolvedRooiamIdentity, RooiamClaims};
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

    // Attach any pending staff invitations for this email to the real account so
    // the user can accept/reject them. We do NOT auto-accept. See docs.
    let _ = crate::repositories::invitation_repository::bind_email_to_user(
        pool.get_ref(),
        &user.email.trim().to_lowercase(),
        user.id,
    )
    .await;

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

// -------------------------------------------------------- SSO token-exchange

#[derive(Debug, Deserialize)]
pub struct CreateSsoSessionRequest {
    pub tenant_slug: String,
    pub token: String,
}

async fn upsert_sso_user(
    pool: &DbPool,
    subject: &str,
    email: &str,
    name: &str,
) -> Result<User, AppError> {
    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (rooiam_subject, email, display_name)
        VALUES ($1, $2, $3)
        ON CONFLICT (rooiam_subject)
        DO UPDATE SET email = EXCLUDED.email, updated_at = NOW()
        RETURNING id, rooiam_subject, email, display_name, avatar_url, created_at, updated_at
        "#,
    )
    .bind(subject)
    .bind(email)
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "database error resolving SSO user");
        AppError::InternalServerError
    })
}

async fn mint_session_token(
    pool: &DbPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<String, AppError> {
    let random = format!(
        "{}{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
        &Uuid::new_v4().simple().to_string()[..8]
    );
    let session_token = format!("{WORKSPACE_SESSION_PREFIX}{random}");
    let expires_at = Utc::now() + Duration::days(30);
    sqlx::query(
        "INSERT INTO workspace_sessions (tenant_id, user_id, token_hash, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(hash_token(&session_token))
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %user_id, "error creating session");
        AppError::InternalServerError
    })?;
    Ok(session_token)
}

/// End-user SSO token-exchange. The customer's backend signs a JWT (HS256) with
/// the workspace's SSO secret; we verify it and mint a workspace session, so
/// their already-signed-in users can participate without any howllo/rooiam
/// login. Identity is namespaced per workspace (`sso:<tenant_id>:<sub>`) so
/// subjects never collide across customers or with rooiam users. See docs/SSO.md.
#[post("/api/auth/sso-session")]
pub async fn create_sso_session(
    pool: web::Data<DbPool>,
    body: web::Json<CreateSsoSessionRequest>,
) -> Result<impl Responder, AppError> {
    let tenant_slug = body.tenant_slug.trim();
    if tenant_slug.is_empty() {
        return Err(AppError::Validation("tenant_slug is required".to_string()));
    }
    let tenant_id =
        crate::repositories::membership_repository::resolve_tenant_id(pool.get_ref(), tenant_slug)
            .await?;

    let secret: Option<String> =
        sqlx::query_scalar("SELECT sso_secret FROM workspace_auth_configs WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_optional(pool.get_ref())
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "error loading sso secret");
                AppError::InternalServerError
            })?
            .flatten();

    let Some(secret) = secret.filter(|value| !value.trim().is_empty()) else {
        return Err(AppError::Validation(
            "SSO is not enabled for this workspace".to_string(),
        ));
    };

    let validation = Validation::new(Algorithm::HS256);
    let claims = decode::<RooiamClaims>(
        body.token.trim(),
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    )
    .map_err(|error| {
        tracing::warn!(error = %error, tenant_slug, "sso token validation failed");
        AppError::Unauthorized
    })?
    .claims;

    if claims.sub.trim().is_empty() {
        return Err(AppError::Unauthorized);
    }

    let subject = format!("sso:{tenant_id}:{}", claims.sub.trim());
    let email = claims
        .email
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{}@sso.local", claims.sub.trim()));
    let name = claims
        .name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Guest".to_string());

    let user = upsert_sso_user(pool.get_ref(), &subject, &email, &name).await?;

    sqlx::query(
        "INSERT INTO memberships (tenant_id, user_id, role) VALUES ($1, $2, 'member') ON CONFLICT (tenant_id, user_id) DO NOTHING",
    )
    .bind(tenant_id)
    .bind(user.id)
    .execute(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "error granting sso membership");
        AppError::InternalServerError
    })?;

    let session_token = mint_session_token(pool.get_ref(), tenant_id, user.id).await?;
    Ok(HttpResponse::Created().json(CreateWorkspaceSessionResponse { session_token }))
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

    // SSO token-exchange: the owner enables SSO, the customer signs a token with
    // the workspace secret, and the end-user gets a session + membership without
    // any rooiam login. A token signed with the wrong secret is rejected.
    #[actix_web::test]
    async fn sso_token_exchange_mints_session_for_customer_user() {
        use chrono::{Duration as ChronoDuration, Utc as ChronoUtc};
        use jsonwebtoken::{encode, EncodingKey, Header};

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

        // Owner enables SSO and gets the workspace secret.
        let admin = bearer_for(&seed.admin_subject, "admin@example.com", "Admin", &settings.rooiam_jwt_secret);
        let regen = read_json(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&format!("/api/admin/sso-config/regenerate?tenant_slug={}", seed.tenant_slug))
                    .insert_header(("Authorization", admin))
                    .to_request(),
            )
            .await,
        )
        .await;
        let secret = regen.get("secret").and_then(|v| v.as_str()).unwrap().to_string();
        assert_eq!(regen.get("enabled").and_then(|v| v.as_bool()), Some(true));

        // The customer's backend signs a token for their already-logged-in user.
        let sign = |secret: &str| {
            let claims = serde_json::json!({
                "sub": "cust-user-1",
                "email": "user@customer.com",
                "name": "Customer User",
                "exp": (ChronoUtc::now() + ChronoDuration::hours(1)).timestamp() as usize,
            });
            encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
        };

        // Exchange it for a howllo session.
        let session = read_json(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/auth/sso-session")
                    .set_json(json!({ "tenant_slug": seed.tenant_slug, "token": sign(&secret) }))
                    .to_request(),
            )
            .await,
        )
        .await;
        let ws_token = session.get("session_token").and_then(|v| v.as_str()).unwrap().to_string();

        // That session can post -> the end-user is a real member.
        let create = test::TestRequest::post()
            .uri(&format!("/api/boards/{}/posts", seed.board_slug))
            .insert_header(("Authorization", format!("Bearer {ws_token}")))
            .set_json(json!({ "tenant_slug": seed.tenant_slug, "title": "From the customer's app", "body": "via SSO" }))
            .to_request();
        assert_eq!(test::call_service(&app, create).await.status(), StatusCode::CREATED);

        // Identity is namespaced per workspace.
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE rooiam_subject LIKE 'sso:%:cust-user-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);

        // A token signed with the wrong secret is rejected.
        let bad = test::TestRequest::post()
            .uri("/api/auth/sso-session")
            .set_json(json!({ "tenant_slug": seed.tenant_slug, "token": sign("wrong-secret") }))
            .to_request();
        assert_eq!(test::call_service(&app, bad).await.status(), StatusCode::UNAUTHORIZED);
    }
}
