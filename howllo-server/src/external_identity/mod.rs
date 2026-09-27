//! Provider-neutral, optional identity directory. Product roles and board
//! permissions continue to be enforced from Howllo's own membership tables.

use actix_web::{delete, get, web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use uuid::Uuid;

use crate::auth::{require_owner, require_permission, AuthenticatedUser};
use crate::config::Settings;
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::errors::AppError;
use crate::repositories::membership_repository;
use crate::audit::{self, AuditEntry};

#[derive(Debug, Deserialize)]
pub struct DirectoryQuery {
    tenant_slug: String,
    page: Option<u32>,
    page_size: Option<u32>,
    q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TenantQuery {
    tenant_slug: String,
}

#[get("/api/admin/identity/status")]
pub async fn provider_status(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    query: web::Query<TenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = authorized_workspace(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    let enabled = linked_workspace(pool.get_ref(), settings.get_ref(), tenant_id).await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(serde_json::json!({ "enabled": enabled })))
}

fn bridge(settings: &Settings) -> Result<(&str, &str, &str), AppError> {
    match (
        settings.identity_bridge_url.as_deref(),
        settings.identity_bridge_token.as_deref(),
        settings.identity_bridge_provider_id.as_deref(),
    ) {
        (Some(url), Some(token), Some(provider))
            if !url.is_empty() && token.len() >= 32 && !provider.is_empty() =>
        {
            let parsed = reqwest::Url::parse(url).map_err(|_| {
                AppError::ServiceUnavailable("Identity directory URL is invalid.".into())
            })?;
            let is_loopback_http = parsed.scheme() == "http"
                && matches!(parsed.host_str(), Some("127.0.0.1" | "::1" | "localhost"));
            if !(parsed.scheme() == "https" || is_loopback_http)
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return Err(AppError::ServiceUnavailable(
                    "Identity directory URL must use HTTPS or loopback HTTP.".into(),
                ));
            }
            Ok((url.trim_end_matches('/'), token, provider))
        }
        (None, None, None) => Err(AppError::NotFound),
        _ => Err(AppError::ServiceUnavailable(
            "Identity directory is not configured correctly.".into(),
        )),
    }
}

async fn authorized_workspace(
    pool: &DbPool,
    tenant_slug: &str,
    actor_user_id: Uuid,
) -> Result<Uuid, AppError> {
    let tenant_id = membership_repository::resolve_tenant_id(pool, tenant_slug).await?;
    require_permission(pool, tenant_id, actor_user_id, Permission::ManageMembers).await?;
    Ok(tenant_id)
}

pub(crate) async fn call_bridge(
    settings: &Settings,
    method: reqwest::Method,
    path: &str,
    query: Option<&[(&str, String)]>,
    body: Option<&Value>,
) -> Result<Value, AppError> {
    let (base, token, _) = bridge(settings)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AppError::InternalServerError)?;
    let mut request = client
        .request(method, format!("{base}{path}"))
        .header("x-howllo-bridge-token", token);
    if let Some(query) = query {
        request = request.query(query);
    }
    if let Some(body) = body {
        request = request.json(body);
    }
    let response = request.send().await.map_err(|error| {
        tracing::warn!(%error, "identity directory request failed");
        AppError::ServiceUnavailable("Identity directory is unavailable.".into())
    })?;
    match response.status().as_u16() {
        401 => return Err(AppError::Unauthorized),
        403 => return Err(AppError::Forbidden),
        404 => return Err(AppError::NotFound),
        429 => {
            return Err(AppError::TooManyRequests(
                "Identity provider rate limit reached.".into(),
            ))
        }
        status if !(200..300).contains(&status) => {
            tracing::warn!(status, "identity directory returned an error");
            return Err(AppError::ServiceUnavailable(
                "Identity directory is unavailable.".into(),
            ));
        }
        _ => {}
    }
    response.json::<Value>().await.map_err(|error| {
        tracing::warn!(%error, "identity directory response was invalid");
        AppError::ServiceUnavailable("Identity directory returned invalid data.".into())
    })
}

/// A missing link means this workspace uses Howllo's own accounts. Any other
/// bridge failure must fail closed instead of switching identity providers.
pub(crate) async fn linked_workspace(pool: &DbPool, settings: &Settings, tenant_id: Uuid) -> Result<bool, AppError> {
    let persisted: Option<String> = sqlx::query_scalar("SELECT provider_id FROM workspace_identity_links WHERE tenant_id=$1")
        .bind(tenant_id).fetch_optional(pool).await.map_err(|_| AppError::InternalServerError)?;
    let (_, _, provider) = match bridge(settings) {
        Ok(configured) => configured,
        Err(AppError::NotFound) if persisted.is_none() => return Ok(false),
        Err(AppError::NotFound) => return Err(AppError::ServiceUnavailable("Linked identity provider is not configured.".into())),
        Err(error) => return Err(error),
    };
    if persisted.as_deref().is_some_and(|selected| selected != provider) {
        return Err(AppError::ServiceUnavailable("Workspace identity provider does not match the configured bridge.".into()));
    }
    let linked = match call_bridge(settings, reqwest::Method::GET,
        &format!("/v1/workspaces/{tenant_id}/link"), None, None).await {
        Ok(value) => value.get("linked").and_then(Value::as_bool) == Some(true),
        Err(AppError::NotFound) if persisted.is_none() => false,
        Err(AppError::NotFound) => return Err(AppError::ServiceUnavailable("Workspace identity mapping is missing.".into())),
        Err(error) => return Err(error),
    };
    if !linked && persisted.is_some() {
        return Err(AppError::ServiceUnavailable("Workspace identity mapping is missing.".into()));
    }
    if linked && persisted.is_none() {
        sqlx::query("INSERT INTO workspace_identity_links(tenant_id,provider_id) VALUES($1,$2) ON CONFLICT(tenant_id) DO NOTHING")
            .bind(tenant_id).bind(provider).execute(pool).await.map_err(|_| AppError::InternalServerError)?;
        let selected: String = sqlx::query_scalar("SELECT provider_id FROM workspace_identity_links WHERE tenant_id=$1")
            .bind(tenant_id).fetch_one(pool).await.map_err(|_| AppError::InternalServerError)?;
        if selected != provider {
            return Err(AppError::ServiceUnavailable("Workspace identity provider changed while linking.".into()));
        }
    }
    Ok(linked)
}

pub(crate) fn provider_id(settings: &Settings) -> Result<&str, AppError> {
    Ok(bridge(settings)?.2)
}

pub(crate) fn encoded_subject(subject: &str) -> Result<String, AppError> {
    if subject.is_empty() { return Err(AppError::Forbidden); }
    let mut url = reqwest::Url::parse("http://localhost/").map_err(|_| AppError::InternalServerError)?;
    url.path_segments_mut().map_err(|_| AppError::InternalServerError)?.push(subject);
    Ok(url.path().trim_start_matches('/').to_string())
}

#[get("/api/admin/identity/members")]
pub async fn list_provider_members(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    query: web::Query<DirectoryQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = authorized_workspace(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    bridge(settings.get_ref())?;
    let q = query.q.as_deref().unwrap_or("").trim();
    if q.len() > 256 {
        return Err(AppError::Validation("Search is too long.".into()));
    }
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let data = call_bridge(
        settings.get_ref(),
        reqwest::Method::GET,
        &format!("/v1/workspaces/{tenant_id}/members"),
        Some(&[
            ("page", page.to_string()),
            ("page_size", page_size.to_string()),
            ("q", q.to_string()),
        ]),
        None,
    )
    .await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(data))
}

#[get("/api/admin/identity/members/{member_id}/sessions")]
pub async fn list_provider_member_sessions(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    path: web::Path<Uuid>,
    query: web::Query<TenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = authorized_workspace(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    let data = call_bridge(
        settings.get_ref(),
        reqwest::Method::GET,
        &format!(
            "/v1/workspaces/{tenant_id}/members/{}/sessions",
            path.into_inner()
        ),
        None,
        None,
    )
    .await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(data))
}

#[delete("/api/admin/identity/members/{member_id}/sessions")]
pub async fn revoke_provider_member_sessions(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    path: web::Path<Uuid>,
    query: web::Query<TenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = authorized_workspace(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    let member_id = path.into_inner();
    require_owner(pool.get_ref(), tenant_id, auth.0.id).await?;
    let (_, _, provider_id) = bridge(settings.get_ref())?;
    let member = call_bridge(
        settings.get_ref(),
        reqwest::Method::GET,
        &format!("/v1/workspaces/{tenant_id}/members/{member_id}"),
        None,
        None,
    )
    .await?;
    let subject = member
        .get("subject")
        .and_then(Value::as_str)
        .filter(|subject| !subject.is_empty())
        .ok_or_else(|| {
            AppError::ServiceUnavailable("Identity directory returned invalid member data.".into())
        })?;
    call_bridge(
        settings.get_ref(),
        reqwest::Method::DELETE,
        &format!("/v1/workspaces/{tenant_id}/members/{member_id}/sessions"),
        None,
        None,
    )
    .await?;
    sqlx::query(
        "UPDATE workspace_sessions SET revoked_at=NOW() WHERE tenant_id=$1 AND revoked_at IS NULL AND user_id IN (SELECT user_id FROM user_identities WHERE provider_id=$2 AND subject=$3)",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .bind(subject)
    .execute(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(%error, "could not revoke linked Howllo workspace sessions");
        AppError::InternalServerError
    })?;
    Ok(HttpResponse::NoContent().finish())
}

#[delete("/api/admin/identity/members/{member_id}")]
pub async fn remove_provider_member(
    pool: web::Data<DbPool>,
    settings: web::Data<Settings>,
    path: web::Path<Uuid>,
    query: web::Query<TenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = authorized_workspace(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    require_owner(pool.get_ref(), tenant_id, auth.0.id).await?;
    if !linked_workspace(pool.get_ref(), settings.get_ref(), tenant_id).await? {
        return Err(AppError::NotFound);
    }
    let member_id = path.into_inner();
    let member = call_bridge(settings.get_ref(), reqwest::Method::GET,
        &format!("/v1/workspaces/{tenant_id}/members/{member_id}"), None, None).await?;
    let subject = member.get("subject").and_then(Value::as_str)
        .filter(|subject| !subject.is_empty())
        .ok_or_else(|| AppError::ServiceUnavailable("Identity provider returned invalid member data.".into()))?;
    let provider = provider_id(settings.get_ref())?;
    let local_user_id: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM user_identities WHERE provider_id=$1 AND subject=$2")
        .bind(provider).bind(subject).fetch_optional(pool.get_ref()).await
        .map_err(|_| AppError::InternalServerError)?;
    if local_user_id == Some(auth.0.id) { return Err(AppError::Forbidden); }
    if let Some(local_user_id) = local_user_id {
        let protected: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM memberships WHERE tenant_id=$1 AND user_id=$2 AND role='owner') OR EXISTS(SELECT 1 FROM account_memberships am JOIN tenants t ON t.account_id=am.account_id WHERE t.id=$1 AND am.user_id=$2 AND am.role IN ('owner','admin'))")
            .bind(tenant_id).bind(local_user_id).fetch_one(pool.get_ref()).await
            .map_err(|_| AppError::InternalServerError)?;
        if protected { return Err(AppError::Forbidden); }
    }
    if let Some(local_user_id) = local_user_id {
        let mut tx = pool.begin().await.map_err(|_| AppError::InternalServerError)?;
        sqlx::query("DELETE FROM memberships WHERE tenant_id=$1 AND user_id=$2")
            .bind(tenant_id).bind(local_user_id).execute(&mut *tx).await
            .map_err(|_| AppError::InternalServerError)?;
        sqlx::query("UPDATE workspace_sessions SET revoked_at=NOW() WHERE tenant_id=$1 AND user_id=$2 AND revoked_at IS NULL")
            .bind(tenant_id).bind(local_user_id).execute(&mut *tx).await
            .map_err(|_| AppError::InternalServerError)?;
        audit::record_in_tx(&mut tx, AuditEntry { tenant_id, actor_user_id: auth.0.id,
            entity_type: "member", entity_id: local_user_id, action: audit::MEMBER_REMOVED,
            old_value: Some(serde_json::json!({"provider_id":provider,"subject":subject})),
            new_value: None, reason: Some("Removed from linked identity workspace".into()) }).await?;
        tx.commit().await.map_err(|_| AppError::InternalServerError)?;
    }
    // Revoke Howllo access first. If the provider is unavailable, retrying this
    // endpoint can finish the provider removal without leaving a live Howllo
    // workspace session behind.
    call_bridge(settings.get_ref(), reqwest::Method::DELETE,
        &format!("/v1/workspaces/{tenant_id}/members/{member_id}"), None, None).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App, HttpRequest, HttpResponse, HttpServer};
    use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
    use uuid::Uuid;

    use crate::db;
    use crate::http::test_support::{
        bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
    };
    use crate::startup;

    #[actix_web::test]
    async fn subject_is_a_single_encoded_path_segment() {
        assert_eq!(super::encoded_subject("a/b?c").unwrap(), "a%2Fb%3Fc");
    }

    #[actix_web::test]
    async fn only_owner_can_remove_linked_member_and_local_access_is_revoked() {
        let _guard = lock_test_db().await;
        let mut settings = test_settings();
        let pool = db::establish_connection(&settings.database_url).await.unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;
        let tenant_id: Uuid = sqlx::query_scalar("SELECT id FROM tenants WHERE slug=$1")
            .bind(&seed.tenant_slug).fetch_one(&pool).await.unwrap();
        sqlx::query("UPDATE memberships SET role='owner' WHERE tenant_id=$1 AND user_id=$2")
            .bind(tenant_id).bind(seed.admin_user_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO user_identities(user_id,provider_id,subject,email) VALUES($1,'rooiam',$2,'member@example.com')")
            .bind(seed.member_user_id).bind(&seed.member_subject).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_sessions(tenant_id,user_id,token_hash,expires_at) VALUES($1,$2,$3,NOW()+INTERVAL '1 day')")
            .bind(tenant_id).bind(seed.member_user_id).bind("linked-member-test-session")
            .execute(&pool).await.unwrap();
        let member_id = Uuid::new_v4();
        let removed = Arc::new(AtomicUsize::new(0));
        let fail_delete = Arc::new(AtomicBool::new(true));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let removed_for_server = removed.clone();
        let fail_for_server = fail_delete.clone();
        let subject = seed.member_subject.clone();
        let server = HttpServer::new(move || {
            let removed = removed_for_server.clone();
            let fail_delete = fail_for_server.clone();
            let subject = subject.clone();
            App::new().app_data(web::Data::new((removed, fail_delete, subject)))
                .default_service(web::to(move |req: HttpRequest, data: web::Data<(Arc<AtomicUsize>, Arc<AtomicBool>, String)>| async move {
                    if req.path().ends_with("/link") { return HttpResponse::Ok().json(serde_json::json!({"linked":true})); }
                    if req.path().ends_with(&format!("/members/{member_id}")) {
                        if req.method() == actix_web::http::Method::DELETE {
                            if data.1.load(Ordering::SeqCst) { return HttpResponse::ServiceUnavailable().finish(); }
                            data.0.fetch_add(1, Ordering::SeqCst);
                            return HttpResponse::Ok().json(serde_json::json!({"ok":true}));
                        }
                        return HttpResponse::Ok().json(serde_json::json!({"id":member_id,"subject":data.2}));
                    }
                    HttpResponse::NotFound().finish()
                }))
        }).listen(listener).unwrap().run();
        let handle = server.handle();
        actix_web::rt::spawn(server);
        settings.identity_bridge_url = Some(format!("http://127.0.0.1:{port}"));
        settings.identity_bridge_token = Some("test-bridge-token-with-at-least-32-characters".into());
        settings.identity_bridge_provider_id = Some("rooiam".into());
        let app = test::init_service(App::new().wrap(crate::http::RequestId)
            .app_data(web::Data::new(pool.clone())).app_data(web::Data::new(settings.clone()))
            .app_data(web::Data::new(crate::realtime::Hub::new())).configure(startup::configure)).await;
        let member = bearer_for(&seed.member_subject, "member@example.com", "Member", &settings.rooiam_jwt_secret);
        let owner = bearer_for(&seed.admin_subject, "admin@example.com", "Admin", &settings.rooiam_jwt_secret);
        let uri = format!("/api/admin/identity/members/{member_id}?tenant_slug={}", seed.tenant_slug);
        assert_eq!(test::call_service(&app, test::TestRequest::delete().uri(&uri)
            .insert_header(("Authorization", member)).to_request()).await.status(), StatusCode::FORBIDDEN);
        assert_eq!(removed.load(Ordering::SeqCst), 0);
        assert_eq!(test::call_service(&app, test::TestRequest::delete().uri(&uri)
            .insert_header(("Authorization", owner.clone())).to_request()).await.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(removed.load(Ordering::SeqCst), 0);
        let has_member: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM memberships WHERE tenant_id=$1 AND user_id=$2)")
            .bind(tenant_id).bind(seed.member_user_id).fetch_one(&pool).await.unwrap();
        assert!(!has_member);
        let revoked: bool = sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM workspace_sessions WHERE token_hash='linked-member-test-session'")
            .fetch_one(&pool).await.unwrap();
        assert!(revoked);
        fail_delete.store(false, Ordering::SeqCst);
        assert_eq!(test::call_service(&app, test::TestRequest::delete().uri(&uri)
            .insert_header(("Authorization", owner)).to_request()).await.status(), StatusCode::NO_CONTENT);
        assert_eq!(removed.load(Ordering::SeqCst), 1);
        handle.stop(true).await;
    }

    #[actix_web::test]
    async fn local_install_hides_external_directory_and_member_cannot_query_it() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;
        let app = test::init_service(
            App::new()
                .wrap(crate::http::RequestId)
                .app_data(web::Data::new(pool))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(crate::realtime::Hub::new()))
                .configure(startup::configure),
        )
        .await;
        let url = format!(
            "/api/admin/identity/status?tenant_slug={}",
            seed.tenant_slug
        );
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let member = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&url)
                .insert_header(("Authorization", admin))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(read_json(response).await["enabled"], false);
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&url)
                .insert_header(("Authorization", member))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
