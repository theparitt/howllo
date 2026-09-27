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
    let enabled = match bridge(settings.get_ref()) {
        Ok(_) => match call_bridge(
            settings.get_ref(),
            reqwest::Method::GET,
            &format!("/v1/workspaces/{tenant_id}/link"),
            None,
        )
        .await
        {
            Ok(value) => value.get("linked").and_then(Value::as_bool) == Some(true),
            Err(AppError::NotFound) => false,
            Err(error) => return Err(error),
        },
        Err(AppError::NotFound) => false,
        Err(error) => return Err(error),
    };
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

async fn call_bridge(
    settings: &Settings,
    method: reqwest::Method,
    path: &str,
    query: Option<&[(&str, String)]>,
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
    let response = request.send().await.map_err(|error| {
        tracing::warn!(%error, "identity directory request failed");
        AppError::ServiceUnavailable("Identity directory is unavailable.".into())
    })?;
    match response.status().as_u16() {
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

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};

    use crate::db;
    use crate::http::test_support::{
        bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
    };
    use crate::startup;

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
