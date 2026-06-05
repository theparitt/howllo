mod actions;
mod repository;
mod service;

use actix_web::{get, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::auth::{require_admin, AuthenticatedUser};
use crate::db::DbPool;
use crate::dto::{AuditLogItemDto, PaginatedResponse};
use crate::errors::AppError;

pub use actions::*;
pub use service::{record, record_in_tx, AuditEntry};

#[derive(Debug, Deserialize)]
pub struct AuditLogListQuery {
    pub tenant_slug: String,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[get("/api/admin/audit-logs")]
pub async fn list_audit_logs(
    pool: web::Data<DbPool>,
    query: web::Query<AuditLogListQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = crate::repositories::membership_repository::resolve_tenant_id(
        pool.get_ref(),
        &query.tenant_slug,
    )
    .await?;

    require_admin(pool.get_ref(), tenant_id, auth.0.id).await?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let total = repository::count_audit_logs(pool.get_ref(), tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(
                error = %error,
                tenant_id = %tenant_id,
                "error counting audit logs"
            );
            AppError::InternalServerError
        })?;

    let items = repository::list_audit_logs(pool.get_ref(), tenant_id, per_page, offset)
        .await
        .map_err(|error| {
            tracing::error!(
                error = %error,
                tenant_id = %tenant_id,
                "error listing audit logs"
            );
            AppError::InternalServerError
        })?
        .into_iter()
        .map(|row| AuditLogItemDto {
            id: row.id,
            tenant_id: row.tenant_id,
            actor_user_id: row.actor_user_id,
            actor_display_name: row.actor_display_name,
            entity_type: row.entity_type,
            entity_id: row.entity_id,
            action: row.action,
            old_value: row.old_value,
            new_value: row.new_value,
            reason: row.reason,
            request_id: row.request_id,
            created_at: row.created_at,
        })
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(PaginatedResponse {
        has_next: page * per_page < total,
        items,
        page,
        per_page,
        total,
    }))
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
    async fn admin_can_view_audit_logs_for_tenant() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let app = test::init_service(
            App::new()
                .wrap(crate::http::RequestId)
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(crate::realtime::Hub::new()))
                .configure(startup::configure),
        )
        .await;

        let admin_token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let create_request = test::TestRequest::post()
            .uri("/api/admin/boards")
            .insert_header(("Authorization", admin_token.clone()))
            .insert_header((crate::http::REQUEST_ID_HEADER, "audit-test-request"))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "slug": "audit-board",
                "name": "Audit Board",
                "description": "Tracked",
                "board_type": "feature-requests",
                "is_private": false
            }))
            .to_request();
        let create_response = test::call_service(&app, create_request).await;
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let list_request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/audit-logs?tenant_slug={}&page=1&per_page=10",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", admin_token))
            .to_request();
        let list_response = test::call_service(&app, list_request).await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let body = read_json(list_response).await;

        assert_eq!(body.get("total").and_then(|value| value.as_i64()), Some(1));
        assert_eq!(
            body.pointer("/items/0/action")
                .and_then(|value| value.as_str()),
            Some("board_created")
        );
        assert_eq!(
            body.pointer("/items/0/entity_type")
                .and_then(|value| value.as_str()),
            Some("board")
        );
        assert_eq!(
            body.pointer("/items/0/request_id")
                .and_then(|value| value.as_str()),
            Some("audit-test-request")
        );
    }

    #[actix_web::test]
    async fn moderator_cannot_view_audit_logs() {
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

        let moderator_token = bearer_for(
            &seed.moderator_subject,
            "moderator@example.com",
            "Moderator",
            &settings.rooiam_jwt_secret,
        );

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/audit-logs?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", moderator_token))
            .to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn audit_events_are_written_for_all_moderation_actions() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let app = test::init_service(
            App::new()
                .wrap(crate::http::RequestId)
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(crate::realtime::Hub::new()))
                .configure(startup::configure),
        )
        .await;

        let admin_token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let admin_header = ("Authorization", admin_token.clone());

        // 1. Create board → board_created
        let r = test::TestRequest::post()
            .uri("/api/admin/boards")
            .insert_header(admin_header.clone())
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "slug": "audit-board-2",
                "name": "Audit Board 2",
                "board_type": "feature-requests",
                "is_private": false
            }))
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let board_json = read_json(resp).await;
        let board_id = board_json.get("id").and_then(|v| v.as_str()).unwrap();

        // 2. Update board → board_updated
        let r = test::TestRequest::patch()
            .uri(&format!("/api/admin/boards/{board_id}"))
            .insert_header(admin_header.clone())
            .set_json(json!({
                "name": "Audit Board 2 Updated",
                "board_type": "feature-requests",
                "is_private": false
            }))
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // 3. Create tag → tag_created
        let _tag_uuid = uuid::Uuid::new_v4().to_string();
        let r = test::TestRequest::post()
            .uri("/api/admin/tags")
            .insert_header(admin_header.clone())
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "slug": "audit-tag",
                "name": "Audit Tag",
                "color": "#999999"
            }))
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let tag_json = read_json(resp).await;
        let tag_id = tag_json.get("id").and_then(|v| v.as_str()).unwrap();

        // 4. Attach tag → tag_attached
        let r = test::TestRequest::post()
            .uri(&format!(
                "/api/admin/posts/{}/tags/{tag_id}",
                seed.canonical_post_id
            ))
            .insert_header(admin_header.clone())
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // 5. Change status → post_status_changed
        let r = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/status",
                seed.canonical_post_id
            ))
            .insert_header(admin_header.clone())
            .set_json(json!({"status": "in_progress", "reason": "starting work"}))
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // 6. Hide post → post_hidden
        let r = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/visibility",
                seed.duplicate_post_id
            ))
            .insert_header(admin_header.clone())
            .set_json(json!({"is_hidden": true}))
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // 7. Soft delete post → post_soft_deleted
        sqlx::query!(
            "UPDATE posts SET deleted_at = NULL, is_hidden = FALSE WHERE id = $1",
            seed.duplicate_post_id
        )
        .execute(&pool)
        .await
        .unwrap();
        let r = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/soft-delete",
                seed.duplicate_post_id
            ))
            .insert_header(admin_header.clone())
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // 8. Create webhook → webhook_created
        let r = test::TestRequest::post()
            .uri("/api/admin/webhooks")
            .insert_header(admin_header.clone())
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "url": "http://127.0.0.1:1/audit-hook",
                "secret": "secret-audit"
            }))
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let wh_json = read_json(resp).await;
        let webhook_id = wh_json.get("id").and_then(|v| v.as_str()).unwrap();

        // 9. Deactivate webhook → webhook_deactivated
        let r = test::TestRequest::patch()
            .uri(&format!("/api/admin/webhooks/{webhook_id}/deactivate"))
            .insert_header(admin_header.clone())
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // 10. Create API token → api_token_created
        let r = test::TestRequest::post()
            .uri("/api/admin/api-tokens")
            .insert_header(admin_header.clone())
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "name": "audit-token",
                "scopes": ["posts:read"]
            }))
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let token_json = read_json(resp).await;
        let token_id = token_json.get("id").and_then(|v| v.as_str()).unwrap();

        // 11. Revoke API token → api_token_revoked
        let r = test::TestRequest::patch()
            .uri(&format!("/api/admin/api-tokens/{token_id}/revoke"))
            .insert_header(admin_header.clone())
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // 12. Hide a comment → comment_hidden
        let comment_row = sqlx::query!(
            "SELECT id FROM comments WHERE post_id = $1 LIMIT 1",
            seed.canonical_post_id
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        if let Some(comment) = comment_row {
            let r = test::TestRequest::patch()
                .uri(&format!("/api/admin/comments/{}/visibility", comment.id))
                .insert_header(admin_header.clone())
                .set_json(json!({"is_hidden": true}))
                .to_request();
            let resp = test::call_service(&app, r).await;
            assert_eq!(resp.status(), StatusCode::OK);
        }

        // 13. Detach tag → tag_detached
        let r = test::TestRequest::delete()
            .uri(&format!(
                "/api/admin/posts/{}/tags/{tag_id}",
                seed.canonical_post_id
            ))
            .insert_header(admin_header.clone())
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // 14. Export → export_posts_json
        let r = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/export/posts.json?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(admin_header.clone())
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // 15. Member role change → member_role_changed
        let r = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/members/{}/role?tenant_slug={}",
                seed.member_user_id, seed.tenant_slug
            ))
            .insert_header(admin_header)
            .set_json(json!({"role": "moderator"}))
            .to_request();
        let resp = test::call_service(&app, r).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Now verify audit log contains all of these
        let list_request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/audit-logs?tenant_slug={}&page=1&per_page=50",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", admin_token))
            .to_request();
        let list_response = test::call_service(&app, list_request).await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let body = read_json(list_response).await;
        let total = body.get("total").and_then(|v| v.as_i64()).unwrap();
        assert!(
            total >= 14,
            "expected at least 14 audit events, got {total}"
        );

        let items = body.get("items").and_then(|v| v.as_array()).unwrap();
        let actions: Vec<&str> = items
            .iter()
            .filter_map(|item| item.get("action").and_then(|v| v.as_str()))
            .collect();

        let expected = [
            "board_created",
            "board_updated",
            "tag_created",
            "tag_attached",
            "post_status_changed",
            "post_hidden",
            "post_soft_deleted",
            "webhook_created",
            "webhook_deactivated",
            "api_token_created",
            "api_token_revoked",
            "tag_detached",
            "export_posts_json",
            "member_role_changed",
        ];

        for action in &expected {
            assert!(
                actions.contains(action),
                "expected audit action '{action}' was not found"
            );
        }
    }
}
