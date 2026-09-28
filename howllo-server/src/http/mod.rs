use std::future::{ready, Ready};
use std::net::IpAddr;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::Error;
use futures::Future;
use sha2::{Digest, Sha256};
use tracing::info;
use uuid::Uuid;

use crate::config::Settings;
use crate::db::DbPool;
use crate::errors::AppError;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Accept a single-IP forwarding header only from an explicitly trusted peer.
/// Direct clients cannot choose their IP by adding a forwarding header.
pub fn client_ip(req: &actix_web::HttpRequest) -> Option<IpAddr> {
    let peer = req.peer_addr()?.ip();
    let settings = req
        .app_data::<actix_web::web::Data<Settings>>()
        .map(|value| value.get_ref());
    let trusted = settings
        .map(|value| {
            value
                .trusted_proxy_cidrs
                .iter()
                .any(|cidr| cidr.contains(&peer))
        })
        .unwrap_or_else(|| peer.is_loopback());
    if trusted {
        let header = settings
            .map(|value| value.client_ip_header.as_str())
            .unwrap_or("cf-connecting-ip");
        if let Some(ip) = req
            .headers()
            .get(header)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<IpAddr>().ok())
        {
            return Some(ip);
        }
    }
    Some(peer)
}

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
pub struct PublicWriteRateLimit;

impl PublicWriteRateLimit {
    pub fn new() -> Self {
        Self
    }
}

pub struct PublicWriteRateLimitMiddleware<S> {
    service: Rc<S>,
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
            service: Rc::new(service),
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
        let service = Rc::clone(&self.service);
        let settings = req
            .app_data::<actix_web::web::Data<Settings>>()
            .map(|data| data.get_ref().clone());
        let pool = req
            .app_data::<actix_web::web::Data<DbPool>>()
            .map(|data| data.get_ref().clone());
        let method = req.method().clone();
        let path = req.path().to_string();
        let peer = client_ip(req.request())
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let local_auth = path.starts_with("/api/auth/local/");
        let identity_self_service = path.starts_with("/api/me/identity/");
        let should_limit = matches!(method.as_str(), "POST" | "PATCH" | "DELETE")
            && (is_public_write_path(&path) || local_auth || identity_self_service);

        Box::pin(async move {
            if should_limit {
                if let Some(settings) = settings.as_ref() {
                    if settings.rate_limit_enabled || local_auth || identity_self_service {
                        let pool = pool.ok_or(AppError::InternalServerError)?;
                        let limit = if local_auth || identity_self_service {
                            10
                        } else {
                            settings.public_write_rate_limit.max(1)
                        };
                        // Group dynamic post IDs and board slugs so changing the
                        // URL cannot reset the IP's one-minute allowance.
                        let scope = if local_auth {
                            format!("local-auth:{method}")
                        } else if identity_self_service {
                            format!("identity-self-service:{method}")
                        } else if path.starts_with("/api/boards/") {
                            format!("board-writes:{method}")
                        } else {
                            format!("post-writes:{method}")
                        };
                        take_public_write_slot(&pool, &scope, &peer, limit).await?;
                    }
                }
            }
            service.call(req).await
        })
    }
}

async fn take_public_write_slot(
    pool: &DbPool,
    scope: &str,
    ip: &str,
    limit: u32,
) -> Result<(), AppError> {
    let identity_hash = hex::encode(Sha256::digest(ip.as_bytes()));
    let attempts: i32 = sqlx::query_scalar(
        "INSERT INTO public_write_rate_limits (scope,identity_hash,window_start,attempts)
         VALUES ($1,$2,clock_timestamp(),1)
         ON CONFLICT (scope,identity_hash) DO UPDATE SET
           attempts=CASE WHEN public_write_rate_limits.window_start < clock_timestamp()-INTERVAL '1 minute'
                         THEN 1 ELSE LEAST(public_write_rate_limits.attempts+1,$3) END,
           window_start=CASE WHEN public_write_rate_limits.window_start < clock_timestamp()-INTERVAL '1 minute'
                             THEN clock_timestamp() ELSE public_write_rate_limits.window_start END
         RETURNING attempts"
    )
    .bind(scope)
    .bind(identity_hash)
    .bind(i32::try_from(limit).unwrap_or(i32::MAX).saturating_add(1))
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(%error, "public write rate limit database error");
        AppError::InternalServerError
    })?;
    if attempts > i32::try_from(limit).unwrap_or(i32::MAX) {
        return Err(AppError::TooManyRequests(
            "Too many requests. Try again in a minute.".into(),
        ));
    }
    // Keep the table bounded without adding a cleanup write to every request.
    if Uuid::new_v4().as_u128() % 1024 == 0 {
        let _ = sqlx::query(
            "DELETE FROM public_write_rate_limits WHERE window_start < clock_timestamp()-INTERVAL '1 day'",
        )
        .execute(pool)
        .await;
    }
    Ok(())
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
    async fn forwarded_ip_is_trusted_only_from_local_proxy() {
        let remote = test::TestRequest::default()
            .peer_addr("203.0.113.10:443".parse().unwrap())
            .insert_header(("cf-connecting-ip", "198.51.100.7"))
            .to_http_request();
        assert_eq!(
            super::client_ip(&remote).unwrap().to_string(),
            "203.0.113.10"
        );

        let local_proxy = test::TestRequest::default()
            .peer_addr("127.0.0.1:30000".parse().unwrap())
            .insert_header(("cf-connecting-ip", "198.51.100.7"))
            .to_http_request();
        assert_eq!(
            super::client_ip(&local_proxy).unwrap().to_string(),
            "198.51.100.7"
        );

        let mut settings = super::test_support::test_settings();
        settings.trusted_proxy_cidrs = vec!["172.18.0.0/16".parse().unwrap()];
        settings.client_ip_header = "x-real-ip".into();
        let trusted_docker_proxy = test::TestRequest::default()
            .peer_addr("172.18.0.3:443".parse().unwrap())
            .insert_header(("x-real-ip", "198.51.100.8"))
            .app_data(actix_web::web::Data::new(settings.clone()))
            .to_http_request();
        assert_eq!(
            super::client_ip(&trusted_docker_proxy).unwrap().to_string(),
            "198.51.100.8"
        );
        let untrusted_docker_peer = test::TestRequest::default()
            .peer_addr("172.19.0.3:443".parse().unwrap())
            .insert_header(("x-real-ip", "198.51.100.8"))
            .app_data(actix_web::web::Data::new(settings))
            .to_http_request();
        assert_eq!(
            super::client_ip(&untrusted_docker_peer)
                .unwrap()
                .to_string(),
            "172.19.0.3"
        );
    }

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
                ADD COLUMN IF NOT EXISTS intro_text TEXT,
                ADD COLUMN IF NOT EXISTS allow_votes BOOLEAN NOT NULL DEFAULT TRUE,
                ADD COLUMN IF NOT EXISTS allow_comments BOOLEAN NOT NULL DEFAULT TRUE;

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

            CREATE TABLE IF NOT EXISTS workspace_policies (
                tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
                overrides JSONB NOT NULL DEFAULT '{}'::jsonb,
                storage_cap_mb INT
            );

            CREATE TABLE IF NOT EXISTS workspace_assets (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                relative_path TEXT NOT NULL,
                size_bytes BIGINT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                UNIQUE (tenant_id, relative_path)
            );

            CREATE TABLE IF NOT EXISTS workspace_asset_reconciliations (
                tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
                reconciled_at TIMESTAMPTZ NOT NULL DEFAULT now()
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

            ALTER TABLE workspace_sessions
                ADD COLUMN IF NOT EXISTS provider_access_ciphertext TEXT,
                ADD COLUMN IF NOT EXISTS provider_refresh_ciphertext TEXT,
                ADD COLUMN IF NOT EXISTS provider_access_expires_at TIMESTAMPTZ,
                ADD COLUMN IF NOT EXISTS provider_session_id UUID,
                ADD COLUMN IF NOT EXISTS provider_token_pending_validation BOOLEAN NOT NULL DEFAULT FALSE,
                ADD COLUMN IF NOT EXISTS provider_verified_at TIMESTAMPTZ;

            CREATE TABLE IF NOT EXISTS public_write_rate_limits (
                scope TEXT NOT NULL,
                identity_hash TEXT NOT NULL,
                window_start TIMESTAMPTZ NOT NULL,
                attempts INTEGER NOT NULL CHECK (attempts > 0),
                PRIMARY KEY (scope, identity_hash)
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
                public_write_rate_limits,
                admin_auth_rate_limit,
                local_auth_rate_limits,
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
                workspace_assets,
                workspace_asset_reconciliations,
                workspace_policies,
                system_settings,
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

        sqlx::query("INSERT INTO tenants (id, slug, name, is_published) VALUES ($1, $2, $3, TRUE)")
            .bind(tenant_id)
            .bind(&tenant_slug)
            .bind("Acme")
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
            identity_bridge_url: None,
            identity_bridge_token: None,
            identity_bridge_provider_id: None,
            admin_bootstrap_key: Some("test-bootstrap-key".to_string()),
            allowed_origins: vec!["http://localhost:3000".to_string()],
            trusted_proxy_cidrs: vec!["127.0.0.0/8".parse().unwrap(), "::1/128".parse().unwrap()],
            client_ip_header: "cf-connecting-ip".into(),
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
