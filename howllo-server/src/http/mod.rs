use std::collections::VecDeque;
use std::future::{ready, Ready};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::Error;
use dashmap::DashMap;
use futures::Future;
use tracing::info;
use uuid::Uuid;

use crate::config::Settings;
use crate::errors::AppError;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

tokio::task_local! {
    static CURRENT_REQUEST_ID: String;
}

pub fn current_request_id() -> Option<String> {
    CURRENT_REQUEST_ID.try_with(Clone::clone).ok()
}

pub struct RequestId;

impl<S, B> Transform<S, ServiceRequest> for RequestId
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestIdMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestIdMiddleware { service }))
    }
}

#[derive(Default)]
pub struct PublicWriteRateLimit {
    buckets: Arc<DashMap<String, VecDeque<Instant>>>,
}

impl PublicWriteRateLimit {
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct PublicWriteRateLimitMiddleware<S> {
    service: S,
    buckets: Arc<DashMap<String, VecDeque<Instant>>>,
}

impl<S, B> Transform<S, ServiceRequest> for PublicWriteRateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = PublicWriteRateLimitMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(PublicWriteRateLimitMiddleware {
            service,
            buckets: Arc::clone(&self.buckets),
        }))
    }
}

impl<S, B> Service<ServiceRequest> for PublicWriteRateLimitMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let settings = req
            .app_data::<actix_web::web::Data<Settings>>()
            .map(|data| data.get_ref().clone());
        let method = req.method().clone();
        let path = req.path().to_string();
        let peer = req
            .connection_info()
            .realip_remote_addr()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unknown".to_string());

        let local_auth = path.starts_with("/api/auth/local/");
        let should_limit = matches!(method.as_str(), "POST" | "PATCH" | "DELETE")
            && (is_public_write_path(&path) || local_auth);

        if should_limit {
            if let Some(settings) = settings.as_ref() {
                if settings.rate_limit_enabled || local_auth {
                    let limit = if local_auth {
                        10
                    } else {
                        settings.public_write_rate_limit.max(1) as usize
                    };
                    let now = Instant::now();
                    let window = Duration::from_secs(60);
                    // Do not trust a caller-supplied Forwarded/X-Forwarded-For
                    // header for password attempt limits.
                    let limiter_peer = if local_auth {
                        req.peer_addr()
                            .map(|addr| addr.ip().to_string())
                            .unwrap_or_else(|| peer.clone())
                    } else {
                        peer.clone()
                    };
                    let key = format!("{limiter_peer}:{path}:{method}");
                    let mut bucket = self.buckets.entry(key).or_default();

                    while let Some(front) = bucket.front() {
                        if now.duration_since(*front) >= window {
                            bucket.pop_front();
                        } else {
                            break;
                        }
                    }

                    if bucket.len() >= limit {
                        return Box::pin(async {
                            Err(AppError::TooManyRequests(
                                "public write rate limit exceeded".to_string(),
                            )
                            .into())
                        });
                    }

                    bucket.push_back(now);
                }
            }
        }

        let fut = self.service.call(req);
        Box::pin(fut)
    }
}

fn is_public_write_path(path: &str) -> bool {
    path.starts_with("/api/boards/") || path.starts_with("/api/posts/")
}

pub struct RequestIdMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestIdMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let request_id = req
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let method = req.method().to_string();
        let path = req.path().to_string();
        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = CURRENT_REQUEST_ID.scope(request_id.clone(), fut).await?;
            let status = res.status().as_u16();

            if let Ok(header_value) = HeaderValue::from_str(&request_id) {
                res.headers_mut()
                    .insert(HeaderName::from_static(REQUEST_ID_HEADER), header_value);
            }

            info!(request_id = %request_id, method = %method, path = %path, status, "request completed");

            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, App};

    use crate::startup;

    use super::RequestId;

    #[actix_web::test]
    async fn request_id_is_added_to_response() {
        let app =
            test::init_service(App::new().wrap(RequestId).configure(startup::configure)).await;
        let request = test::TestRequest::get().uri("/api/health").to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key(super::REQUEST_ID_HEADER));
    }
}

#[cfg(test)]
pub mod test_support {
    use actix_web::test;
    use chrono::{Duration, Utc};
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::Value;
    use sqlx::Executor;
    use std::sync::OnceLock;
    use tokio::sync::{Mutex, MutexGuard};
    use uuid::Uuid;

    use crate::auth::RooiamClaims;
    use crate::config::{AiProvider, AiSettings, Settings};
    use crate::db::DbPool;

    static TEST_DB_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    pub async fn lock_test_db<'a>() -> MutexGuard<'a, ()> {
        TEST_DB_MUTEX.get_or_init(|| Mutex::new(())).lock().await
    }

    pub async fn reset_db(pool: &DbPool) {
        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS system_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL DEFAULT '',
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            ALTER TABLE tenants
            ADD COLUMN IF NOT EXISTS default_board_enabled BOOLEAN NOT NULL DEFAULT TRUE;

            ALTER TABLE boards
            ADD COLUMN IF NOT EXISTS is_default BOOLEAN NOT NULL DEFAULT FALSE;

            ALTER TABLE boards
            ADD COLUMN IF NOT EXISTS icon_url TEXT;

            ALTER TABLE boards
            ADD COLUMN IF NOT EXISTS background_color VARCHAR(32);

            ALTER TABLE boards
            ADD COLUMN IF NOT EXISTS dashboard_sections TEXT[] NOT NULL DEFAULT '{progress,latest,top}';

            ALTER TABLE users
            ADD COLUMN IF NOT EXISTS avatar_url TEXT;

            CREATE TABLE IF NOT EXISTS tenant_branding (
                tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
                site_name VARCHAR(255),
                logo_url TEXT,
                accent_color VARCHAR(32),
                background_color VARCHAR(32),
                show_powered_by BOOLEAN NOT NULL DEFAULT TRUE,
                show_roadmap BOOLEAN NOT NULL DEFAULT TRUE,
                show_boards BOOLEAN NOT NULL DEFAULT TRUE,
                show_feed BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            ALTER TABLE tenant_branding
                ADD COLUMN IF NOT EXISTS show_roadmap BOOLEAN NOT NULL DEFAULT TRUE;

            ALTER TABLE tenant_branding
                ADD COLUMN IF NOT EXISTS show_boards BOOLEAN NOT NULL DEFAULT TRUE,
                ADD COLUMN IF NOT EXISTS show_feed BOOLEAN NOT NULL DEFAULT TRUE;

            ALTER TABLE tenant_branding
                ADD COLUMN IF NOT EXISTS require_post_approval BOOLEAN NOT NULL DEFAULT FALSE;

            ALTER TABLE tenant_branding
                ADD COLUMN IF NOT EXISTS posts_per_hour INT NOT NULL DEFAULT 3,
                ADD COLUMN IF NOT EXISTS comments_per_hour INT NOT NULL DEFAULT 15,
                ADD COLUMN IF NOT EXISTS board_posts_per_10m INT NOT NULL DEFAULT 20,
                ADD COLUMN IF NOT EXISTS board_comments_per_10m INT NOT NULL DEFAULT 60;

            CREATE TABLE IF NOT EXISTS board_write_cooldowns (
                board_id UUID NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                kind TEXT NOT NULL,
                until_at TIMESTAMPTZ NOT NULL,
                PRIMARY KEY (board_id, kind)
            );

            CREATE TABLE IF NOT EXISTS workspace_spam_flags (
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                until_at TIMESTAMPTZ NOT NULL,
                reason TEXT NOT NULL,
                PRIMARY KEY (tenant_id, user_id)
            );

            ALTER TABLE posts
                ADD COLUMN IF NOT EXISTS review_state TEXT NOT NULL DEFAULT 'approved';

            ALTER TABLE posts
                ADD COLUMN IF NOT EXISTS review_reason TEXT;

            ALTER TABLE boards
                ADD COLUMN IF NOT EXISTS is_enabled BOOLEAN NOT NULL DEFAULT TRUE;

            CREATE TABLE IF NOT EXISTS workspace_auth_configs (
                tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
                provider VARCHAR(32) NOT NULL DEFAULT 'rooiam',
                rooiam_workspace_id TEXT,
                rooiam_client_id TEXT,
                rooiam_widget_base_url TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            ALTER TABLE workspace_auth_configs
            ADD COLUMN IF NOT EXISTS sso_secret TEXT;

            CREATE TABLE IF NOT EXISTS workspace_sessions (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                token_hash TEXT NOT NULL UNIQUE,
                expires_at TIMESTAMPTZ NOT NULL,
                revoked_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                last_used_at TIMESTAMPTZ
            );

            ALTER TABLE posts
            ADD COLUMN IF NOT EXISTS attachments JSONB NOT NULL DEFAULT '[]'::jsonb;

            ALTER TABLE post_follows
            ADD COLUMN IF NOT EXISTS notify_on_comment BOOLEAN NOT NULL DEFAULT TRUE;

            CREATE TABLE IF NOT EXISTS accounts (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                slug VARCHAR(255) UNIQUE NOT NULL,
                name VARCHAR(255) NOT NULL,
                owner_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS account_memberships (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                role VARCHAR(50) NOT NULL DEFAULT 'owner',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE (account_id, user_id)
            );

            ALTER TABLE tenants
            ADD COLUMN IF NOT EXISTS account_id UUID REFERENCES accounts(id) ON DELETE CASCADE;

            CREATE TABLE IF NOT EXISTS workspace_invitations (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                email TEXT NOT NULL,
                role VARCHAR(50) NOT NULL,
                status VARCHAR(20) NOT NULL DEFAULT 'pending',
                invited_by UUID REFERENCES users(id) ON DELETE SET NULL,
                user_id UUID REFERENCES users(id) ON DELETE SET NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                responded_at TIMESTAMPTZ,
                expires_at TIMESTAMPTZ
            );
            CREATE UNIQUE INDEX IF NOT EXISTS workspace_invitations_one_pending_idx
                ON workspace_invitations (tenant_id, lower(email)) WHERE status = 'pending';

            ALTER TABLE memberships
                ADD COLUMN IF NOT EXISTS public_participant BOOLEAN NOT NULL DEFAULT FALSE;

            CREATE TABLE IF NOT EXISTS workspace_restrictions (
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                kind TEXT NOT NULL,
                reason TEXT NOT NULL DEFAULT '',
                expires_at TIMESTAMPTZ,
                created_by UUID REFERENCES users(id) ON DELETE SET NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (tenant_id, user_id)
            );

            DROP INDEX IF EXISTS boards_one_default_per_tenant_idx;
            "#,
        )
        .await
        .unwrap();

        pool.execute(
            r#"
            TRUNCATE TABLE
                audit_logs,
                ai_suggestions,
                notifications,
                moderation_notes,
                webhook_deliveries,
                webhook_events,
                webhook_endpoints,
                api_tokens,
                post_follows,
                post_status_history,
                post_tags,
                tags,
                post_votes,
                board_write_cooldowns,
                workspace_spam_flags,
                comments,
                posts,
                workspace_invitations,
                workspace_restrictions,
                boards,
                memberships,
                account_memberships,
                accounts,
                users,
                tenants,
                tenant_branding,
                workspace_auth_configs,
                workspace_sessions
            RESTART IDENTITY CASCADE
            "#,
        )
        .await
        .unwrap();
    }

    pub async fn seed_basic_tenant(pool: &DbPool) -> SeedData {
        let tenant_id = Uuid::new_v4();
        let board_id = Uuid::new_v4();
        let private_board_id = Uuid::new_v4();
        let admin_user_id = Uuid::new_v4();
        let moderator_user_id = Uuid::new_v4();
        let member_user_id = Uuid::new_v4();
        let canonical_post_id = Uuid::new_v4();
        let duplicate_post_id = Uuid::new_v4();
        let private_post_id = Uuid::new_v4();
        let tenant_slug = format!("acme-{}", Uuid::new_v4().simple());
        let board_slug = format!("features-{}", Uuid::new_v4().simple());
        let private_board_slug = format!("internal-{}", Uuid::new_v4().simple());
        let admin_subject = format!("admin-{}", Uuid::new_v4().simple());
        let moderator_subject = format!("moderator-{}", Uuid::new_v4().simple());
        let member_subject = format!("member-{}", Uuid::new_v4().simple());
        let admin_email = format!("{admin_subject}@example.com");
        let moderator_email = format!("{moderator_subject}@example.com");
        let member_email = format!("{member_subject}@example.com");

        sqlx::query!(
            "INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)",
            tenant_id,
            tenant_slug,
            "Acme"
        )
        .execute(pool)
        .await
        .unwrap();

        sqlx::query!(
            "INSERT INTO users (id, rooiam_subject, email, display_name) VALUES ($1, $2, $3, $4), ($5, $6, $7, $8), ($9, $10, $11, $12)",
            admin_user_id,
            admin_subject,
            admin_email,
            "Admin",
            moderator_user_id,
            moderator_subject,
            moderator_email,
            "Moderator",
            member_user_id,
            member_subject,
            member_email,
            "Member"
        )
        .execute(pool)
        .await
        .unwrap();

        sqlx::query!(
            "INSERT INTO memberships (tenant_id, user_id, role) VALUES ($1, $2, $3), ($1, $4, $5), ($1, $6, $7)",
            tenant_id,
            admin_user_id,
            "admin",
            moderator_user_id,
            "moderator",
            member_user_id,
            "member"
        )
        .execute(pool)
        .await
        .unwrap();

        sqlx::query!(
            "INSERT INTO boards (id, tenant_id, slug, name, board_type, is_private) VALUES ($1, $2, $3, $4, $5, FALSE), ($6, $2, $7, $8, $9, TRUE)",
            board_id,
            tenant_id,
            board_slug,
            "Features",
            "feature-requests",
            private_board_id,
            private_board_slug,
            "Internal",
            "feature-requests"
        )
        .execute(pool)
        .await
        .unwrap();

        sqlx::query!(
            r#"
            INSERT INTO posts (id, tenant_id, board_id, user_id, title, body, status)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7),
                ($8, $2, $3, $9, $10, $11, $12),
                ($13, $2, $14, $4, $15, $16, $17)
            "#,
            canonical_post_id,
            tenant_id,
            board_id,
            admin_user_id,
            "Dark Mode",
            "Need dark mode",
            "planned",
            duplicate_post_id,
            member_user_id,
            "Theme Dark",
            "Please add dark theme",
            "under_review",
            private_post_id,
            private_board_id,
            "Secret Roadmap",
            "Internal planning",
            "in_progress"
        )
        .execute(pool)
        .await
        .unwrap();

        SeedData {
            tenant_slug,
            board_slug,
            private_board_slug,
            admin_user_id,
            moderator_user_id,
            member_user_id,
            admin_subject,
            moderator_subject,
            member_subject,
            canonical_post_id,
            duplicate_post_id,
            private_post_id,
        }
    }

    pub fn test_settings() -> Settings {
        Settings {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@localhost:5433/howllo".to_string()
            }),
            bind_address: "127.0.0.1:0".to_string(),
            rooiam_jwt_secret: "dev-secret".to_string(),
            admin_jwt_secret: "dev-secret".to_string(),
            rooiam_legacy_hs256_enabled: true,
            rooiam_hosted_userinfo_url: None,
            rooiam_widget_base_url: Some("https://api.rooiam.com/login-widget".to_string()),
            rooiam_widget_workspace_id: Some("workspace-dev".to_string()),
            rooiam_widget_client_id: Some("client-dev".to_string()),
            workspace_auth_provider: "local".to_string(),
            admin_bootstrap_key: Some("test-bootstrap-key".to_string()),
            allowed_origins: vec!["http://localhost:3000".to_string()],
            rate_limit_enabled: false,
            public_write_rate_limit: 60,
            max_post_body_chars: 10_000,
            max_comment_body_chars: 4_000,
            webhook_timeout_ms: 5_000,
            ai: AiSettings {
                enabled: false,
                provider: AiProvider::Disabled,
                base_url: "http://127.0.0.1:11434".to_string(),
                model: "gemma4".to_string(),
            },
        }
    }

    pub fn bearer_for(subject: &str, email: &str, name: &str, secret: &str) -> String {
        let claims = RooiamClaims {
            sub: subject.to_string(),
            exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
            email: Some(email.to_string()),
            name: Some(name.to_string()),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();

        format!("Bearer {token}")
    }

    pub async fn read_json(response: actix_web::dev::ServiceResponse) -> Value {
        test::read_body_json(response).await
    }

    pub struct SeedData {
        pub tenant_slug: String,
        pub board_slug: String,
        pub private_board_slug: String,
        pub admin_user_id: Uuid,
        pub moderator_user_id: Uuid,
        pub member_user_id: Uuid,
        pub admin_subject: String,
        pub moderator_subject: String,
        pub member_subject: String,
        pub canonical_post_id: Uuid,
        pub duplicate_post_id: Uuid,
        pub private_post_id: Uuid,
    }
}
