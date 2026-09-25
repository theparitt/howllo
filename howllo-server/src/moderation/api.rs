use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::dto::{
    UpdateCommentVisibilityRequest, UpdateLockRequest, UpdateOfficialCommentRequest,
    UpdatePostDuplicateRequest, UpdatePostStatusRequest, UpdateVisibilityRequest,
};
use crate::errors::AppError;
use crate::realtime::Hub;
use crate::services::moderation_service;
use actix_web::{get, patch, web, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ReviewPostRequest {
    pub action: String,
}

#[derive(Deserialize)]
pub struct ModerationQueueQuery {
    pub tenant_slug: String,
    pub board_slug: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[get("/api/admin/moderation/queue")]
pub async fn get_moderation_queue(
    pool: web::Data<DbPool>,
    query: web::Query<ModerationQueueQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 100);
    let (items, total) = moderation_service::get_moderation_queue(
        pool.get_ref(),
        &query.tenant_slug,
        query.board_slug.as_deref(),
        page,
        per_page,
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(crate::dto::PaginatedResponse {
        has_next: page * per_page < total,
        items,
        page,
        per_page,
        total,
    }))
}

#[patch("/api/admin/posts/{post_id}/status")]
pub async fn update_post_status(
    pool: web::Data<DbPool>,
    hub: web::Data<Hub>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdatePostStatusRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    moderation_service::update_post_status(
        pool.get_ref(),
        hub.get_ref(),
        post_id,
        &body,
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/api/admin/posts/{post_id}/visibility")]
pub async fn update_post_visibility(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdateVisibilityRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    moderation_service::update_post_visibility(pool.get_ref(), path.into_inner(), &body, auth.0.id)
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/api/admin/posts/{post_id}/review")]
pub async fn review_post(
    pool: web::Data<DbPool>,
    hub: web::Data<Hub>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<ReviewPostRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    moderation_service::review_post(
        pool.get_ref(),
        hub.get_ref(),
        path.into_inner(),
        &body.action,
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/api/admin/posts/{post_id}/soft-delete")]
pub async fn soft_delete_post(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    moderation_service::soft_delete_post(pool.get_ref(), path.into_inner(), auth.0.id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/api/admin/posts/{post_id}/restore")]
pub async fn restore_post(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    moderation_service::restore_post(pool.get_ref(), path.into_inner(), auth.0.id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/api/admin/posts/{post_id}/lock")]
pub async fn update_post_lock(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdateLockRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    moderation_service::update_post_lock(pool.get_ref(), path.into_inner(), &body, auth.0.id)
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/api/admin/comments/{comment_id}/official")]
pub async fn update_comment_official(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdateOfficialCommentRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    moderation_service::update_comment_official(
        pool.get_ref(),
        path.into_inner(),
        &body,
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/api/admin/comments/{comment_id}/visibility")]
pub async fn update_comment_visibility(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdateCommentVisibilityRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    moderation_service::update_comment_visibility(
        pool.get_ref(),
        path.into_inner(),
        &body,
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/api/admin/posts/{post_id}/duplicate")]
pub async fn update_post_duplicate(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdatePostDuplicateRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    moderation_service::update_post_duplicate(pool.get_ref(), path.into_inner(), &body, auth.0.id)
        .await?;
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
    async fn admin_can_mark_post_as_duplicate() {
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

        let request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/duplicate",
                seed.duplicate_post_id
            ))
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({ "duplicate_of_post_id": seed.canonical_post_id }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);

        let detail_request = test::TestRequest::get()
            .uri(&format!(
                "/api/posts/{}?tenant_slug={}",
                seed.duplicate_post_id, seed.tenant_slug
            ))
            .to_request();
        let detail_response = test::call_service(&app, detail_request).await;
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail_json = read_json(detail_response).await;
        let canonical_id = seed.canonical_post_id.to_string();

        assert_eq!(
            detail_json
                .get("duplicate_of_post_id")
                .and_then(|v| v.as_str()),
            Some(canonical_id.as_str())
        );
    }

    #[actix_web::test]
    async fn admin_can_soft_delete_post() {
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

        let delete_request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/soft-delete",
                seed.duplicate_post_id
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let delete_response = test::call_service(&app, delete_request).await;
        assert_eq!(delete_response.status(), StatusCode::OK);

        let detail_request = test::TestRequest::get()
            .uri(&format!(
                "/api/posts/{}?tenant_slug={}",
                seed.duplicate_post_id, seed.tenant_slug
            ))
            .to_request();
        let detail_response = test::call_service(&app, detail_request).await;
        assert_eq!(detail_response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn moderator_can_mark_post_as_duplicate_but_not_change_status() {
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

        let duplicate_request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/duplicate",
                seed.duplicate_post_id
            ))
            .insert_header(("Authorization", moderator_token.clone()))
            .set_json(json!({ "duplicate_of_post_id": seed.canonical_post_id }))
            .to_request();
        let duplicate_response = test::call_service(&app, duplicate_request).await;
        assert_eq!(duplicate_response.status(), StatusCode::OK);

        let status_request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/status",
                seed.canonical_post_id
            ))
            .insert_header(("Authorization", moderator_token))
            .set_json(json!({ "status": "done", "reason": "moderators cannot do this" }))
            .to_request();
        let status_response = test::call_service(&app, status_request).await;
        assert_eq!(status_response.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn admin_cannot_move_declined_directly_to_done() {
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

        let set_declined = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/status",
                seed.canonical_post_id
            ))
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({ "status": "declined", "reason": "won't do" }))
            .to_request();
        let resp = test::call_service(&app, set_declined).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let invalid_transition = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/status",
                seed.canonical_post_id
            ))
            .insert_header(("Authorization", token))
            .set_json(json!({ "status": "done", "reason": "trying to skip" }))
            .to_request();
        let resp = test::call_service(&app, invalid_transition).await;
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[actix_web::test]
    async fn duplicate_cannot_point_to_hidden_post() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET is_hidden = TRUE WHERE id = $1",
            seed.canonical_post_id
        )
        .execute(&pool)
        .await
        .unwrap();

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

        let request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/duplicate",
                seed.duplicate_post_id
            ))
            .insert_header(("Authorization", token))
            .set_json(json!({ "duplicate_of_post_id": seed.canonical_post_id }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[actix_web::test]
    async fn duplicate_cannot_point_to_deleted_post() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET deleted_at = NOW() WHERE id = $1",
            seed.canonical_post_id
        )
        .execute(&pool)
        .await
        .unwrap();

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

        let request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/duplicate",
                seed.duplicate_post_id
            ))
            .insert_header(("Authorization", token))
            .set_json(json!({ "duplicate_of_post_id": seed.canonical_post_id }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[actix_web::test]
    async fn duplicate_cannot_create_chain() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET duplicate_of_post_id = $1 WHERE id = $2",
            seed.canonical_post_id,
            seed.duplicate_post_id
        )
        .execute(&pool)
        .await
        .unwrap();

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

        let request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/duplicate",
                seed.canonical_post_id
            ))
            .insert_header(("Authorization", token))
            .set_json(json!({ "duplicate_of_post_id": seed.duplicate_post_id }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[actix_web::test]
    async fn declined_requires_reason() {
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

        let request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/status",
                seed.canonical_post_id
            ))
            .insert_header(("Authorization", token))
            .set_json(json!({ "status": "declined" }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[actix_web::test]
    async fn moderator_cannot_manage_members() {
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
                "/api/admin/members?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", moderator_token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn moderation_queue_includes_hidden_posts() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET is_hidden = TRUE WHERE id = $1",
            seed.duplicate_post_id
        )
        .execute(&pool)
        .await
        .unwrap();

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

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/moderation/queue?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert!(body.get("total").and_then(|v| v.as_i64()).unwrap_or(0) >= 1);
    }

    #[actix_web::test]
    async fn locked_post_rejects_new_comments() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET is_locked = TRUE WHERE id = $1",
            seed.canonical_post_id
        )
        .execute(&pool)
        .await
        .unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(crate::realtime::Hub::new()))
                .configure(startup::configure),
        )
        .await;

        let token = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );

        let request = test::TestRequest::post()
            .uri(&format!("/api/posts/{}/comments", seed.canonical_post_id))
            .insert_header(("Authorization", token))
            .set_json(json!({ "body": "locked post comment" }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn cross_tenant_cannot_mark_duplicate() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tenant_b_id = uuid::Uuid::new_v4();
        let post_b_id = uuid::Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO tenants (id, slug, name) VALUES ($1, 'tenant-b-3', 'B')",
            tenant_b_id
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO posts (id, tenant_id, board_id, user_id, title, body, status) SELECT $1, $2, b.id, u.id, 'Post B', 'b', 'planned' FROM boards b JOIN users u ON u.rooiam_subject = $3 WHERE b.tenant_id = $2 LIMIT 1",
            post_b_id,
            tenant_b_id,
            seed.admin_subject,
        )
        .execute(&pool)
        .await
        .unwrap();

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

        let request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/duplicate",
                seed.duplicate_post_id
            ))
            .insert_header(("Authorization", token))
            .set_json(json!({ "duplicate_of_post_id": post_b_id }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert!(response.status().is_client_error());
    }
}
