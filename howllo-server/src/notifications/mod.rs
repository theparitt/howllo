use actix_web::{get, patch, web, HttpResponse, Responder};
use serde::Serialize;

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::repositories::notification_repository;

#[derive(Debug, Serialize)]
pub struct NotificationDto {
    pub id: uuid::Uuid,
    pub event_type: String,
    pub title: String,
    pub body: String,
    pub is_read: bool,
    pub post_id: Option<uuid::Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct FollowNotification<'a> {
    pub event_type: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub notify_column: &'a str,
}

pub async fn create_follow_notifications(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    post_id: uuid::Uuid,
    actor_user_id: uuid::Uuid,
    notification: FollowNotification<'_>,
) -> Result<(), AppError> {
    notification_repository::create_follow_notifications(
        pool,
        tenant_id,
        post_id,
        notification.event_type,
        notification.title,
        notification.body,
        actor_user_id,
        notification.notify_column,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, "error creating follow notifications");
        AppError::InternalServerError
    })
}

#[get("/api/notifications")]
pub async fn list_notifications(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let items = notification_repository::list_notifications(pool.get_ref(), auth.0.id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %auth.0.id, "error listing notifications");
            AppError::InternalServerError
        })?;

    Ok(HttpResponse::Ok().json(items))
}

#[patch("/api/notifications/{notification_id}/read")]
pub async fn mark_notification_read(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let notification_id = path.into_inner();

    notification_repository::mark_notification_read(pool.get_ref(), notification_id, auth.0.id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, notification_id = %notification_id, user_id = %auth.0.id, "error marking notification read");
            AppError::InternalServerError
        })?;

    Ok(HttpResponse::Ok().finish())
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
    async fn follower_gets_notification_after_status_change() {
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

        let member_token = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );
        let follow_request = test::TestRequest::post()
            .uri(&format!("/api/posts/{}/follow", seed.canonical_post_id))
            .insert_header(("Authorization", member_token.clone()))
            .to_request();
        let follow_response = test::call_service(&app, follow_request).await;
        assert_eq!(follow_response.status(), StatusCode::OK);

        let admin_token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let status_request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/status",
                seed.canonical_post_id
            ))
            .insert_header(("Authorization", admin_token))
            .set_json(json!({"status": "in_progress", "reason": "started work"}))
            .to_request();
        let status_response = test::call_service(&app, status_request).await;
        assert_eq!(status_response.status(), StatusCode::OK);

        let notifications_request = test::TestRequest::get()
            .uri("/api/notifications")
            .insert_header(("Authorization", member_token))
            .to_request();
        let notifications_response = test::call_service(&app, notifications_request).await;
        assert_eq!(notifications_response.status(), StatusCode::OK);
        let body = read_json(notifications_response).await;
        let items = body.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].get("event_type").and_then(|v| v.as_str()),
            Some("status_changed")
        );
    }
}
