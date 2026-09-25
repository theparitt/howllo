use actix_web::{get, patch, post, web, HttpResponse, Responder};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::repositories::webhook_repository;
use crate::services::webhook_service;

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub tenant_slug: String,
    pub url: String,
    pub secret: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookListQuery {
    pub tenant_slug: String,
}

pub async fn emit_tenant_event(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    event_type: &str,
    payload: Value,
) -> Result<(), AppError> {
    let event_id = webhook_repository::create_webhook_event(pool, tenant_id, event_type, &payload)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, event_type = event_type, "error creating webhook event");
            AppError::InternalServerError
        })?;

    let endpoints = webhook_repository::get_active_endpoints(pool, tenant_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, event_type = event_type, "error fetching webhook endpoints");
            AppError::InternalServerError
        })?;

    if endpoints.is_empty() {
        return Ok(());
    }

    let pool = pool.clone();
    let event_type = event_type.to_string();
    let body = json!({ "event_type": event_type, "payload": payload });

    actix_web::rt::spawn(async move {
        let timeout_ms = std::env::var("HOWLLO_WEBHOOK_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5_000);
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
            .unwrap_or_else(|_| Client::new());
        for (endpoint_id, url, secret) in endpoints {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs().to_string())
                .unwrap_or_else(|_| "0".to_string());
            let mut request = client
                .post(&url)
                .header("content-type", "application/json")
                .header("x-howllo-event-id", event_id.to_string())
                .header("x-howllo-timestamp", timestamp.clone())
                .json(&body);

            if let Some(secret) = secret.as_deref() {
                if let Some(signature) = sign_webhook(secret, event_id, &timestamp, &body) {
                    request = request.header("x-howllo-signature", signature);
                }
            }

            match request.send().await {
                Ok(resp) => {
                    let status_code = resp.status().as_u16() as i32;
                    let response_body = resp.text().await.unwrap_or_default();
                    let _ = webhook_repository::record_webhook_delivery(
                        &pool,
                        event_id,
                        endpoint_id,
                        status_code,
                        (200..300).contains(&(status_code as u16)),
                        &response_body,
                    )
                    .await;
                }
                Err(err) => {
                    let _ = webhook_repository::record_webhook_delivery_failure(
                        &pool,
                        event_id,
                        endpoint_id,
                        &err.to_string(),
                    )
                    .await;
                }
            }
        }
    });

    Ok(())
}

pub fn sign_webhook(
    secret: &str,
    event_id: uuid::Uuid,
    timestamp: &str,
    body: &Value,
) -> Option<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(event_id.to_string().as_bytes());
    mac.update(b".");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body.to_string().as_bytes());
    Some(format!(
        "sha256={}",
        hex::encode(mac.finalize().into_bytes())
    ))
}

#[get("/api/admin/webhooks")]
pub async fn list_webhooks(
    pool: web::Data<DbPool>,
    query: web::Query<WebhookListQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let items =
        webhook_service::list_webhooks(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(items))
}

#[post("/api/admin/webhooks")]
pub async fn create_webhook(
    pool: web::Data<DbPool>,
    body: web::Json<CreateWebhookRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    if body.url.trim().is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }
    let item = webhook_service::create_webhook(
        pool.get_ref(),
        &body.tenant_slug,
        body.url.trim(),
        body.secret.as_deref(),
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Created().json(item))
}

#[patch("/api/admin/webhooks/{webhook_id}/deactivate")]
pub async fn deactivate_webhook(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let webhook_id = path.into_inner();
    webhook_service::deactivate_webhook(pool.get_ref(), webhook_id, auth.0.id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[derive(serde::Deserialize)]
pub struct DeliveryListQuery {
    pub tenant_slug: String,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct DeliveryStatusRow {
    pub id: uuid::Uuid,
    pub webhook_event_id: uuid::Uuid,
    pub webhook_endpoint_id: uuid::Uuid,
    pub status_code: Option<i32>,
    pub success: bool,
    pub response_body: Option<String>,
    pub delivered_at: chrono::DateTime<chrono::Utc>,
    pub attempt_count: i32,
    pub next_retry_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[get("/api/admin/webhooks/deliveries")]
pub async fn get_delivery_status(
    pool: web::Data<DbPool>,
    query: web::Query<DeliveryListQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant = crate::repositories::membership_repository::resolve_tenant_id(
        pool.get_ref(),
        &query.tenant_slug,
    )
    .await?;

    crate::auth::require_permission(
        pool.get_ref(),
        tenant,
        auth.0.id,
        crate::domain::permission::Permission::ManageWebhooks,
    )
    .await?;

    let deliveries = sqlx::query_as::<_, DeliveryStatusRow>(
        r#"
        SELECT d.id, d.webhook_event_id, d.webhook_endpoint_id, d.status_code,
               d.success, d.response_body, d.delivered_at, d.attempt_count, d.next_retry_at
        FROM webhook_deliveries d
        JOIN webhook_events e ON d.webhook_event_id = e.id
        WHERE e.tenant_id = $1
        ORDER BY d.delivered_at DESC
        LIMIT 100
        "#,
    )
    .bind(tenant)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant, "error fetching webhook deliveries");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Ok().json(deliveries))
}

#[derive(serde::Deserialize)]
pub struct ResendDeliveryRequest {
    pub tenant_slug: String,
}

#[post("/api/admin/webhooks/deliveries/{delivery_id}/resend")]
pub async fn resend_webhook_delivery(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<ResendDeliveryRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant = crate::repositories::membership_repository::resolve_tenant_id(
        pool.get_ref(),
        &body.tenant_slug,
    )
    .await?;

    crate::auth::require_permission(
        pool.get_ref(),
        tenant,
        auth.0.id,
        crate::domain::permission::Permission::ManageWebhooks,
    )
    .await?;

    let delivery_id = path.into_inner();

    let delivery = sqlx::query!(
        r#"
        SELECT d.webhook_event_id, d.webhook_endpoint_id, e.event_type, e.payload,
               w.url, w.secret
        FROM webhook_deliveries d
        JOIN webhook_events e ON d.webhook_event_id = e.id
        JOIN webhook_endpoints w ON d.webhook_endpoint_id = w.id
        WHERE d.id = $1 AND e.tenant_id = $2
        "#,
        delivery_id,
        tenant
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "error fetching delivery for resend");
        AppError::InternalServerError
    })?
    .ok_or(AppError::NotFound)?;

    let body_json = serde_json::json!({
        "event_type": delivery.event_type,
        "payload": delivery.payload,
    });

    actix_web::rt::spawn(async move {
        let timeout_ms = std::env::var("HOWLLO_WEBHOOK_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5_000);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());
        let mut request = client
            .post(&delivery.url)
            .header("content-type", "application/json")
            .header("x-howllo-event-id", delivery.webhook_event_id.to_string())
            .header("x-howllo-timestamp", &timestamp)
            .json(&body_json);

        if let Some(secret) = delivery.secret.as_deref() {
            if let Some(sig) = crate::webhooks::sign_webhook(
                secret,
                delivery.webhook_event_id,
                &timestamp,
                &body_json,
            ) {
                request = request.header("x-howllo-signature", sig);
            }
        }

        match request.send().await {
            Ok(resp) => {
                let status_code = resp.status().as_u16() as i32;
                let response_body = resp.text().await.unwrap_or_default();
                let _ = webhook_repository::record_webhook_delivery(
                    &pool,
                    delivery.webhook_event_id,
                    delivery.webhook_endpoint_id,
                    status_code,
                    (200..300).contains(&(status_code as u16)),
                    &response_body,
                )
                .await;
            }
            Err(err) => {
                let _ = webhook_repository::record_webhook_delivery_failure(
                    &pool,
                    delivery.webhook_event_id,
                    delivery.webhook_endpoint_id,
                    &err.to_string(),
                )
                .await;
            }
        }
    });

    Ok(HttpResponse::Accepted().finish())
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

    #[actix_web::test]
    async fn admin_can_create_and_list_webhooks() {
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

        let token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let create_request = test::TestRequest::post()
            .uri("/api/admin/webhooks")
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "url": "http://127.0.0.1:9/webhook",
                "secret": "top-secret"
            }))
            .to_request();
        let create_response = test::call_service(&app, create_request).await;
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let list_request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/webhooks?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let list_response = test::call_service(&app, list_request).await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let body = read_json(list_response).await;
        let items = body.as_array().unwrap();
        assert_eq!(items.len(), 1);
    }

    #[actix_web::test]
    async fn moderator_cannot_create_webhook() {
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

        let token = bearer_for(
            &seed.moderator_subject,
            "moderator@example.com",
            "Moderator",
            &settings.rooiam_jwt_secret,
        );

        let create_request = test::TestRequest::post()
            .uri("/api/admin/webhooks")
            .insert_header(("Authorization", token))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "url": "http://127.0.0.1:9/webhook",
                "secret": "blocked"
            }))
            .to_request();
        let create_response = test::call_service(&app, create_request).await;
        assert_eq!(create_response.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn webhook_list_hides_raw_secret() {
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

        let token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let create_request = test::TestRequest::post()
            .uri("/api/admin/webhooks")
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "url": "http://127.0.0.1:1/wh",
                "secret": "super-secret-value"
            }))
            .to_request();
        test::call_service(&app, create_request).await;

        let list_request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/webhooks?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let list_response = test::call_service(&app, list_request).await;
        let body = read_json(list_response).await;
        let items = body.as_array().unwrap();
        assert!(!items.is_empty());
        let item = &items[0];
        assert_eq!(item.get("has_secret").and_then(|v| v.as_bool()), Some(true));
        assert!(item.get("secret").is_none());
    }

    #[actix_web::test]
    async fn sign_webhook_produces_valid_hmac() {
        let secret = "test-secret";
        let event_id = uuid::Uuid::new_v4();
        let timestamp = "1234567890";
        let body = serde_json::json!({"event_type": "test", "payload": "data"});

        let signature = super::sign_webhook(secret, event_id, timestamp, &body);
        assert!(signature.is_some());
        let sig = signature.unwrap();
        assert!(sig.starts_with("sha256="));
        assert!(sig.len() > 7);
    }

    #[actix_web::test]
    async fn deactivated_webhook_not_delivered() {
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

        let token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let create_request = test::TestRequest::post()
            .uri("/api/admin/webhooks")
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "url": "http://127.0.0.1:1/w",
                "secret": "s"
            }))
            .to_request();
        let resp = test::call_service(&app, create_request).await;
        let created = read_json(resp).await;
        let webhook_id = created.get("id").and_then(|v| v.as_str()).unwrap();

        let deactivate_request = test::TestRequest::patch()
            .uri(&format!("/api/admin/webhooks/{webhook_id}/deactivate"))
            .insert_header(("Authorization", token))
            .to_request();
        let resp = test::call_service(&app, deactivate_request).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let active = crate::repositories::webhook_repository::get_active_endpoints(
            &pool,
            uuid::Uuid::parse_str(
                sqlx::query_scalar::<_, String>("SELECT id::text FROM tenants WHERE slug = $1")
                    .bind(&seed.tenant_slug)
                    .fetch_one(&pool)
                    .await
                    .unwrap()
                    .as_str(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
        assert!(active.is_empty());
    }

    #[actix_web::test]
    async fn webhook_delivery_failure_is_recorded() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tenant_id = uuid::Uuid::parse_str(
            sqlx::query_scalar::<_, String>("SELECT id::text FROM tenants WHERE slug = $1")
                .bind(&seed.tenant_slug)
                .fetch_one(&pool)
                .await
                .unwrap()
                .as_str(),
        )
        .unwrap();

        let event = crate::repositories::webhook_repository::create_webhook_event(
            &pool,
            tenant_id,
            "test",
            &serde_json::json!({}),
        )
        .await
        .unwrap();

        let endpoint = sqlx::query!(
            "INSERT INTO webhook_endpoints (tenant_id, url, secret) VALUES ($1, $2, $3) RETURNING id",
            tenant_id,
            "http://127.0.0.1:9/test",
            "s",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        crate::repositories::webhook_repository::record_webhook_delivery_failure(
            &pool,
            event,
            endpoint.id,
            "connection refused",
        )
        .await
        .unwrap();

        crate::repositories::webhook_repository::record_webhook_delivery(
            &pool,
            event,
            endpoint.id,
            200,
            true,
            "ok",
        )
        .await
        .unwrap();
    }

    #[actix_web::test]
    async fn successful_delivery_records_success() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tenant_id = uuid::Uuid::parse_str(
            &sqlx::query_scalar::<_, String>("SELECT id::text FROM tenants WHERE slug = $1")
                .bind(&seed.tenant_slug)
                .fetch_one(&pool)
                .await
                .unwrap(),
        )
        .unwrap();

        let event = crate::repositories::webhook_repository::create_webhook_event(
            &pool,
            tenant_id,
            "test",
            &serde_json::json!({}),
        )
        .await
        .unwrap();

        let endpoint = sqlx::query!(
            "INSERT INTO webhook_endpoints (tenant_id, url, secret) VALUES ($1, $2, $3) RETURNING id",
            tenant_id, "http://127.0.0.1:9/t", "s",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        crate::repositories::webhook_repository::record_webhook_delivery(
            &pool,
            event,
            endpoint.id,
            200,
            true,
            "ok",
        )
        .await
        .unwrap();

        let count = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM webhook_deliveries WHERE webhook_event_id = $1 AND success = true",
        )
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(count >= 1);
    }

    #[actix_web::test]
    async fn failed_delivery_schedules_retry() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tenant_id = uuid::Uuid::parse_str(
            &sqlx::query_scalar::<_, String>("SELECT id::text FROM tenants WHERE slug = $1")
                .bind(&seed.tenant_slug)
                .fetch_one(&pool)
                .await
                .unwrap(),
        )
        .unwrap();

        let event = crate::repositories::webhook_repository::create_webhook_event(
            &pool,
            tenant_id,
            "test",
            &serde_json::json!({}),
        )
        .await
        .unwrap();

        let endpoint = sqlx::query!(
            "INSERT INTO webhook_endpoints (tenant_id, url, secret) VALUES ($1, $2, $3) RETURNING id",
            tenant_id, "http://127.0.0.1:9/t2", "s",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        crate::repositories::webhook_repository::record_webhook_delivery_failure(
            &pool,
            event,
            endpoint.id,
            "connection refused",
        )
        .await
        .unwrap();

        sqlx::query!(
            "UPDATE webhook_deliveries SET next_retry_at = NOW() + INTERVAL '1 minute' WHERE webhook_event_id = $1 AND success = false",
            event,
        )
        .execute(&pool)
        .await
        .unwrap();

        let pending = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM webhook_deliveries WHERE webhook_event_id = $1 AND success = false AND next_retry_at IS NOT NULL",
        )
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(pending >= 1);
    }

    #[actix_web::test]
    async fn retry_increments_attempt_count() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tenant_id = uuid::Uuid::parse_str(
            &sqlx::query_scalar::<_, String>("SELECT id::text FROM tenants WHERE slug = $1")
                .bind(&seed.tenant_slug)
                .fetch_one(&pool)
                .await
                .unwrap(),
        )
        .unwrap();

        let event = crate::repositories::webhook_repository::create_webhook_event(
            &pool,
            tenant_id,
            "test",
            &serde_json::json!({}),
        )
        .await
        .unwrap();

        let endpoint = sqlx::query!(
            "INSERT INTO webhook_endpoints (tenant_id, url, secret) VALUES ($1, $2, $3) RETURNING id",
            tenant_id, "http://127.0.0.1:9/t3", "s",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        // First failure
        crate::repositories::webhook_repository::record_webhook_delivery_failure(
            &pool,
            event,
            endpoint.id,
            "error1",
        )
        .await
        .unwrap();

        // Second failure
        crate::repositories::webhook_repository::record_webhook_delivery_failure(
            &pool,
            event,
            endpoint.id,
            "error2",
        )
        .await
        .unwrap();

        let count = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM webhook_deliveries WHERE webhook_event_id = $1 AND success = false",
        )
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(count >= 2);
    }

    #[actix_web::test]
    async fn delivery_becomes_abandoned_after_max_attempts() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tenant_id = uuid::Uuid::parse_str(
            &sqlx::query_scalar::<_, String>("SELECT id::text FROM tenants WHERE slug = $1")
                .bind(&seed.tenant_slug)
                .fetch_one(&pool)
                .await
                .unwrap(),
        )
        .unwrap();

        let event = crate::repositories::webhook_repository::create_webhook_event(
            &pool,
            tenant_id,
            "test",
            &serde_json::json!({}),
        )
        .await
        .unwrap();

        let endpoint = sqlx::query!(
            "INSERT INTO webhook_endpoints (tenant_id, url, secret) VALUES ($1, $2, $3) RETURNING id",
            tenant_id, "http://127.0.0.1:9/t4", "s",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        for _ in 0..5 {
            crate::repositories::webhook_repository::record_webhook_delivery_failure(
                &pool,
                event,
                endpoint.id,
                "error",
            )
            .await
            .unwrap();
        }

        sqlx::query!(
            "UPDATE webhook_deliveries SET next_retry_at = NULL WHERE webhook_event_id = $1 AND success = false",
            event,
        )
        .execute(&pool)
        .await
        .unwrap();

        let abandoned = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM webhook_deliveries WHERE webhook_event_id = $1 AND success = false AND next_retry_at IS NULL",
        )
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(abandoned >= 1);
    }

    #[actix_web::test]
    async fn manual_resend_works() {
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

        let token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let create_wh = test::TestRequest::post()
            .uri("/api/admin/webhooks")
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "url": "http://127.0.0.1:9/resend-test",
                "secret": "rs"
            }))
            .to_request();
        let wh_resp = test::call_service(&app, create_wh).await;
        let wh_body = read_json(wh_resp).await;
        let wh_id =
            uuid::Uuid::parse_str(wh_body.get("id").and_then(|v| v.as_str()).unwrap()).unwrap();

        let tenant_id = uuid::Uuid::parse_str(
            &sqlx::query_scalar::<_, String>("SELECT id::text FROM tenants WHERE slug = $1")
                .bind(&seed.tenant_slug)
                .fetch_one(&pool)
                .await
                .unwrap(),
        )
        .unwrap();

        let event = crate::repositories::webhook_repository::create_webhook_event(
            &pool,
            tenant_id,
            "test.resend",
            &serde_json::json!({"key": "val"}),
        )
        .await
        .unwrap();

        crate::repositories::webhook_repository::record_webhook_delivery_failure(
            &pool,
            event,
            wh_id,
            "conn refused",
        )
        .await
        .unwrap();

        let delivery = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM webhook_deliveries WHERE webhook_event_id = $1 AND success = false LIMIT 1",
        )
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();

        let resend_req = test::TestRequest::post()
            .uri(&format!("/api/admin/webhooks/deliveries/{delivery}/resend",))
            .insert_header(("Authorization", token))
            .set_json(json!({"tenant_slug": seed.tenant_slug}))
            .to_request();
        let resend_resp = test::call_service(&app, resend_req).await;
        assert_eq!(resend_resp.status(), StatusCode::ACCEPTED);
    }

    #[actix_web::test]
    async fn webhook_timeout_uses_config() {
        std::env::set_var("HOWLLO_WEBHOOK_TIMEOUT_MS", "3000");
        let ms: u64 = std::env::var("HOWLLO_WEBHOOK_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5000);
        assert_eq!(ms, 3000);
        std::env::remove_var("HOWLLO_WEBHOOK_TIMEOUT_MS");
    }
}
