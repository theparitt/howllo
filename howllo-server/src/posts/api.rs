use actix_web::{get, patch, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::auth::{require_optional_api_token_scope, AuthenticatedUser};
use crate::db::DbPool;
use crate::dto::{
    CreatePostRequest, PaginatedResponse, PostListItemDto, StatusHistoryItemDto, UpdatePostRequest,
};
use crate::errors::AppError;
use crate::realtime::{Hub, RealtimeEvent};
use crate::repositories::post_repository;
use crate::services::{post_service, roadmap_service};

#[derive(Deserialize)]
pub struct PostListQuery {
    pub tenant_slug: String,
    pub sort: Option<String>,
    pub status: Option<String>,
    pub tag: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    /// Admin-only: include hidden and soft-deleted posts in the result so they
    /// can be reviewed and restored. Requires tenant admin membership.
    pub include_hidden: Option<bool>,
}

#[cfg(test)]
mod review_tests {
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;

    use crate::db;
    use crate::http::test_support::{
        bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
    };
    use crate::startup;

    #[actix_web::test]
    async fn held_post_is_private_until_approved_and_burst_is_limited() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;
        sqlx::query("UPDATE posts SET created_at = now() - interval '2 days' WHERE user_id = $1")
            .bind(seed.member_user_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tenant_branding (tenant_id, require_post_approval) SELECT id, TRUE FROM tenants WHERE slug = $1")
            .bind(&seed.tenant_slug)
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
        let member = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );
        let moderator = bearer_for(
            &seed.moderator_subject,
            "moderator@example.com",
            "Moderator",
            &settings.rooiam_jwt_secret,
        );

        let path = format!("/api/boards/{}/posts", seed.board_slug);
        let request = test::TestRequest::post().uri(&path)
            .insert_header(("Authorization", member.clone()))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "title": "Please add export", "body": "Export would help our team."}))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let created = read_json(response).await;
        assert_eq!(created["review_state"], "pending");
        let id = created["id"].as_str().unwrap();

        let request = test::TestRequest::get()
            .uri(&format!("/api/posts/{id}?tenant_slug={}", seed.tenant_slug))
            .to_request();
        assert_eq!(
            test::call_service(&app, request).await.status(),
            StatusCode::NOT_FOUND
        );

        let request = test::TestRequest::post().uri(&path)
            .insert_header(("Authorization", member.clone()))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "title": "Another request", "body": "Another idea."}))
            .to_request();
        assert_eq!(
            test::call_service(&app, request).await.status(),
            StatusCode::TOO_MANY_REQUESTS
        );

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/moderation/queue?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", moderator.clone()))
            .to_request();
        let queue = read_json(test::call_service(&app, request).await).await;
        assert!(queue["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == id));

        let request = test::TestRequest::patch()
            .uri(&format!("/api/admin/posts/{id}/review"))
            .insert_header(("Authorization", moderator))
            .set_json(json!({"action": "approve"}))
            .to_request();
        assert_eq!(
            test::call_service(&app, request).await.status(),
            StatusCode::OK
        );
        let request = test::TestRequest::get()
            .uri("/api/notifications")
            .insert_header(("Authorization", member))
            .to_request();
        let notifications = read_json(test::call_service(&app, request).await).await;
        assert!(notifications
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["event_type"] == "post_approved"));
        let request = test::TestRequest::get()
            .uri(&format!("/api/posts/{id}?tenant_slug={}", seed.tenant_slug))
            .to_request();
        assert_eq!(
            test::call_service(&app, request).await.status(),
            StatusCode::OK
        );
    }

    #[actix_web::test]
    async fn normal_post_publishes_but_link_heavy_post_waits_for_review() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;
        sqlx::query("UPDATE posts SET created_at = now() - interval '2 days' WHERE user_id = $1")
            .bind(seed.member_user_id)
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
        let path = format!("/api/boards/{}/posts", seed.board_slug);

        let request = test::TestRequest::post().uri(&path)
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "title": "Ordinary idea", "body": "This would help."}))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let created = read_json(response).await;
        assert_eq!(created["review_state"], "approved");

        let approved_id = created["id"].as_str().unwrap();
        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/posts/{approved_id}?tenant_slug={}",
                seed.tenant_slug
            ))
            .to_request();
        assert_eq!(
            test::call_service(&app, request).await.status(),
            StatusCode::OK
        );

        sqlx::query("UPDATE posts SET created_at = now() - interval '1 minute' WHERE id = $1")
            .bind(uuid::Uuid::parse_str(approved_id).unwrap())
            .execute(&pool)
            .await
            .unwrap();
        let request = test::TestRequest::post().uri(&path)
            .insert_header(("Authorization", token))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "title": "Links to check", "body": "https://example.com and https://example.org"}))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let held = read_json(response).await;
        assert_eq!(held["review_state"], "pending");

        let moderator = bearer_for(
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
            .insert_header(("Authorization", moderator))
            .to_request();
        let queue = read_json(test::call_service(&app, request).await).await;
        assert!(queue["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == held["id"]));
        assert!(!queue["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == approved_id));
    }
}

#[derive(Deserialize)]
pub struct PostDetailQuery {
    pub tenant_slug: String,
}

#[derive(Deserialize)]
pub struct PostStatusHistoryQuery {
    pub tenant_slug: String,
}

#[get("/api/boards/{board_slug}/posts")]
pub async fn list_board_posts(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    query: web::Query<PostListQuery>,
) -> Result<impl Responder, AppError> {
    let _ = require_optional_api_token_scope(&req, "posts:read").await?;
    let board_slug = path.into_inner();
    if query.tenant_slug.trim().is_empty() {
        return Err(AppError::Validation("tenant_slug is required".to_string()));
    }

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
    let sort = query.sort.as_deref().unwrap_or("newest");

    let response: PaginatedResponse<PostListItemDto> =
        post_service::list_board_posts(post_service::BoardPostListInput {
            board_slug: &board_slug,
            include_hidden: query.include_hidden.unwrap_or(false),
            page,
            per_page,
            pool: pool.get_ref(),
            req: &req,
            sort,
            status: query.status.as_deref(),
            tag: query.tag.as_deref(),
            tenant_slug: query.tenant_slug.trim(),
        })
        .await?;

    Ok(HttpResponse::Ok().json(response))
}

#[get("/api/posts/{post_id}")]
pub async fn get_post_detail(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<PostDetailQuery>,
) -> Result<impl Responder, AppError> {
    let _ = require_optional_api_token_scope(&req, "posts:read").await?;
    let post_id = path.into_inner();
    let post =
        post_service::get_post_detail(&req, pool.get_ref(), post_id, &query.tenant_slug).await?;
    Ok(HttpResponse::Ok().json(post))
}

#[get("/api/posts/{post_id}/status-history")]
pub async fn get_post_status_history(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<PostStatusHistoryQuery>,
) -> Result<impl Responder, AppError> {
    let _ = require_optional_api_token_scope(&req, "posts:read").await?;
    let post_id = path.into_inner();
    let history: Vec<StatusHistoryItemDto> =
        post_service::get_post_status_history(&req, pool.get_ref(), post_id, &query.tenant_slug)
            .await?;
    Ok(HttpResponse::Ok().json(history))
}

#[post("/api/boards/{board_slug}/posts")]
pub async fn create_post(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    hub: web::Data<Hub>,
    path: web::Path<String>,
    body: web::Json<CreatePostRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let board_slug = path.into_inner();
    let user_id = auth.0.id;
    body.validate()?;
    let settings = req
        .app_data::<web::Data<crate::config::Settings>>()
        .ok_or(AppError::InternalServerError)?;

    if body.body.chars().count() > settings.max_post_body_chars {
        return Err(AppError::Validation(format!(
            "body must be {} characters or fewer",
            settings.max_post_body_chars
        )));
    }

    let post = post_service::create_post(
        &req,
        pool.get_ref(),
        &body.tenant_slug,
        &board_slug,
        &body.title,
        &body.body,
        &body.attachments,
        user_id,
    )
    .await?;

    if post.review_state == "approved" {
        if let Some(scope) = post_repository::find_post_access(pool.get_ref(), post.id, None)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, post_id = %post.id, "error loading post scope for realtime");
            AppError::InternalServerError
        })?
    {
        hub.broadcast(
            scope.tenant_id,
            RealtimeEvent {
                event_type: "post.created".to_string(),
                board_id: Some(scope.board_id),
                post_id: Some(post.id),
            },
        );
    }
    }

    Ok(HttpResponse::Created().json(post))
}

#[patch("/api/posts/{post_id}")]
pub async fn update_post(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdatePostRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    body.validate()?;
    let settings = req
        .app_data::<web::Data<crate::config::Settings>>()
        .ok_or(AppError::InternalServerError)?;
    if body.body.chars().count() > settings.max_post_body_chars {
        return Err(AppError::Validation(format!(
            "body must be {} characters or fewer",
            settings.max_post_body_chars
        )));
    }
    post_service::update_post(pool.get_ref(), post_id, &body, auth.0.id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
pub struct RoadmapQuery {
    pub tenant_slug: String,
    pub tag: Option<String>,
    pub status: Option<String>,
    pub board_slug: Option<String>,
}

async fn fetch_roadmap_items(
    req: &HttpRequest,
    pool: &DbPool,
    tenant_slug: &str,
    status_filter: Option<&str>,
    tag_filter: Option<&str>,
    board_slug_filter: Option<&str>,
) -> Result<Vec<PostListItemDto>, AppError> {
    roadmap_service::get_roadmap_items(
        req,
        pool,
        tenant_slug,
        status_filter,
        tag_filter,
        board_slug_filter,
    )
    .await
}

#[get("/api/roadmap")]
pub async fn get_roadmap(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    query: web::Query<RoadmapQuery>,
) -> Result<impl Responder, AppError> {
    let _ = require_optional_api_token_scope(&req, "roadmap:read").await?;
    let items = fetch_roadmap_items(
        &req,
        pool.get_ref(),
        &query.tenant_slug,
        query.status.as_deref(),
        query.tag.as_deref(),
        query.board_slug.as_deref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(items))
}

#[get("/api/roadmap/by-status")]
pub async fn get_roadmap_by_status(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    query: web::Query<RoadmapQuery>,
) -> Result<impl Responder, AppError> {
    let _ = require_optional_api_token_scope(&req, "roadmap:read").await?;
    let items = fetch_roadmap_items(
        &req,
        pool.get_ref(),
        &query.tenant_slug,
        query.status.as_deref(),
        query.tag.as_deref(),
        query.board_slug.as_deref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(items))
}

#[get("/api/roadmap/by-tag")]
pub async fn get_roadmap_by_tag(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    query: web::Query<RoadmapQuery>,
) -> Result<impl Responder, AppError> {
    let _ = require_optional_api_token_scope(&req, "roadmap:read").await?;
    let items = fetch_roadmap_items(
        &req,
        pool.get_ref(),
        &query.tenant_slug,
        query.status.as_deref(),
        query.tag.as_deref(),
        query.board_slug.as_deref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(items))
}

#[get("/api/roadmap/grouped")]
pub async fn get_roadmap_grouped(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    query: web::Query<RoadmapQuery>,
) -> Result<impl Responder, AppError> {
    let _ = require_optional_api_token_scope(&req, "roadmap:read").await?;
    let planned = fetch_roadmap_items(
        &req,
        pool.get_ref(),
        &query.tenant_slug,
        Some("planned"),
        query.tag.as_deref(),
        query.board_slug.as_deref(),
    )
    .await?;
    let in_progress = fetch_roadmap_items(
        &req,
        pool.get_ref(),
        &query.tenant_slug,
        Some("in_progress"),
        query.tag.as_deref(),
        query.board_slug.as_deref(),
    )
    .await?;
    let done = fetch_roadmap_items(
        &req,
        pool.get_ref(),
        &query.tenant_slug,
        Some("done"),
        query.tag.as_deref(),
        query.board_slug.as_deref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(crate::dto::RoadmapGroupedDto {
        planned,
        in_progress,
        done,
    }))
}

#[derive(serde::Deserialize)]
pub struct PostActivityQuery {
    pub tenant_slug: String,
}

#[get("/api/posts/{post_id}/activity")]
pub async fn get_post_activity(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<PostActivityQuery>,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    let _ = require_optional_api_token_scope(&req, "posts:read").await?;

    post_service::ensure_post_read_access(&req, pool.get_ref(), post_id, &query.tenant_slug)
        .await?;

    let status_history =
        post_service::get_post_status_history(&req, pool.get_ref(), post_id, &query.tenant_slug)
            .await?;

    let official_response =
        crate::repositories::post_repository::get_official_response(pool.get_ref(), post_id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, post_id = %post_id, "error fetching official response");
                AppError::InternalServerError
            })?;

    let post_detail = crate::repositories::post_repository::find_post_detail(
        pool.get_ref(),
        post_id,
        &query.tenant_slug,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, post_id = %post_id, "error fetching post detail");
        AppError::InternalServerError
    })?
    .ok_or(AppError::NotFound)?;

    let follower_count =
        crate::repositories::post_repository::get_post_follower_count(pool.get_ref(), post_id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, post_id = %post_id, "error counting followers");
                AppError::InternalServerError
            })?;

    Ok(HttpResponse::Ok().json(crate::dto::PostActivityDto {
        status_history,
        official_response,
        duplicate_of_post_id: post_detail.duplicate_of_post_id,
        follower_count,
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
    async fn status_history_endpoint_returns_admin_transition() {
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

        let update_request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/posts/{}/status",
                seed.canonical_post_id
            ))
            .insert_header(("Authorization", token))
            .set_json(json!({ "status": "in_progress", "reason": "started implementation" }))
            .to_request();
        let update_response = test::call_service(&app, update_request).await;
        assert_eq!(update_response.status(), StatusCode::OK);

        let history_request = test::TestRequest::get()
            .uri(&format!(
                "/api/posts/{}/status-history?tenant_slug={}",
                seed.canonical_post_id, seed.tenant_slug
            ))
            .to_request();
        let history_response = test::call_service(&app, history_request).await;
        assert_eq!(history_response.status(), StatusCode::OK);

        let history_json = read_json(history_response).await;
        let items = history_json.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].get("old_status").and_then(|v| v.as_str()),
            Some("planned")
        );
        assert_eq!(
            items[0].get("new_status").and_then(|v| v.as_str()),
            Some("in_progress")
        );
        assert_eq!(
            items[0].get("actor_display_name").and_then(|v| v.as_str()),
            Some("Admin")
        );
        assert_eq!(
            items[0].get("reason").and_then(|v| v.as_str()),
            Some("started implementation")
        );
    }

    #[actix_web::test]
    async fn post_list_supports_status_filter_and_pagination() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let extra_post_id = uuid::Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO posts (id, tenant_id, board_id, user_id, title, body, status)
            SELECT $1, t.id, b.id, $2, $3, $4, $5
            FROM tenants t
            JOIN boards b ON b.tenant_id = t.id
            WHERE t.slug = $6 AND b.slug = $7
            "#,
            extra_post_id,
            seed.admin_user_id,
            "Second Planned Item",
            "More work to do",
            "planned",
            seed.tenant_slug,
            seed.board_slug
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

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/boards/{}/posts?tenant_slug={}&status=planned&page=1&per_page=1&sort=top",
                seed.board_slug, seed.tenant_slug
            ))
            .to_request();
        let response = test::call_service(&app, request).await;
        let status = response.status();
        if status != StatusCode::OK {
            let body = actix_web::test::read_body(response).await;
            panic!(
                "expected 200, got {} with body {}",
                status,
                String::from_utf8_lossy(&body)
            );
        }

        let body = read_json(response).await;
        let items = body.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(body.get("page").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(body.get("per_page").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(body.get("total").and_then(|v| v.as_i64()), Some(2));
        assert_eq!(body.get("has_next").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            items[0].get("status").and_then(|v| v.as_str()),
            Some("planned")
        );
    }

    #[actix_web::test]
    async fn author_can_edit_own_post() {
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
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );

        let update_request = test::TestRequest::patch()
            .uri(&format!("/api/posts/{}", seed.duplicate_post_id))
            .insert_header(("Authorization", token))
            .set_json(json!({ "title": "Theme Dark Updated", "body": "Updated body" }))
            .to_request();
        let update_response = test::call_service(&app, update_request).await;
        assert_eq!(update_response.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn post_detail_includes_tags_follow_state_and_official_response() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tag_id = uuid::Uuid::new_v4();
        let comment_id = uuid::Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO tags (id, tenant_id, slug, name, color) SELECT $1, id, $2, $3, $4 FROM tenants WHERE slug = $5",
            tag_id,
            "priority",
            "Priority",
            "#ff0000",
            seed.tenant_slug
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO post_tags (post_id, tag_id) VALUES ($1, $2)",
            seed.canonical_post_id,
            tag_id
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO post_follows (post_id, user_id) VALUES ($1, $2)",
            seed.canonical_post_id,
            seed.member_user_id
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            r#"
            INSERT INTO comments (id, post_id, user_id, body, is_official_response, comment_type)
            VALUES ($1, $2, $3, $4, TRUE, 'official')
            "#,
            comment_id,
            seed.canonical_post_id,
            seed.admin_user_id,
            "We are working on this now."
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

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/posts/{}?tenant_slug={}",
                seed.canonical_post_id, seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert_eq!(
            body.get("author_display_name").and_then(|v| v.as_str()),
            Some("Admin")
        );
        assert_eq!(body.get("comment_count").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(
            body.get("tags")
                .and_then(|v| v.as_array())
                .and_then(|tags| tags.first())
                .and_then(|tag| tag.get("slug"))
                .and_then(|v| v.as_str()),
            Some("priority")
        );
        assert_eq!(
            body.get("follow_state")
                .and_then(|v| v.get("is_following"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            body.get("official_response")
                .and_then(|v| v.get("body"))
                .and_then(|v| v.as_str()),
            Some("We are working on this now.")
        );
    }

    #[actix_web::test]
    async fn moderator_can_list_hidden_posts_with_include_hidden() {
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
                "/api/boards/{}/posts?tenant_slug={}&include_hidden=true",
                seed.board_slug, seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = read_json(response).await;
        assert_eq!(body.get("total").and_then(|v| v.as_i64()), Some(2));
        assert!(body
            .get("items")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .any(|item| item.get("is_hidden").and_then(|v| v.as_bool()) == Some(true)));
    }

    #[actix_web::test]
    async fn author_cannot_edit_hidden_post() {
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
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );

        let request = test::TestRequest::patch()
            .uri(&format!("/api/posts/{}", seed.duplicate_post_id))
            .insert_header(("Authorization", token))
            .set_json(json!({ "title": "Updated", "body": "Updated body" }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn author_cannot_edit_soft_deleted_post() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET deleted_at = NOW() WHERE id = $1",
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
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );

        let request = test::TestRequest::patch()
            .uri(&format!("/api/posts/{}", seed.duplicate_post_id))
            .insert_header(("Authorization", token))
            .set_json(json!({ "title": "Updated", "body": "Updated body" }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn roadmap_status_filter_returns_only_matching_status() {
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

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/roadmap?tenant_slug={}&status=planned",
                seed.tenant_slug
            ))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        assert!(items
            .iter()
            .all(|item| { item.get("status").and_then(|v| v.as_str()) == Some("planned") }));
    }

    #[actix_web::test]
    async fn roadmap_tag_filter_returns_only_matching_tag() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tag_id = uuid::Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO tags (id, tenant_id, slug, name, color) SELECT $1, id, $2, $3, $4 FROM tenants WHERE slug = $5",
            tag_id,
            "security",
            "Security",
            "#ff0000",
            seed.tenant_slug
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO post_tags (post_id, tag_id) VALUES ($1, $2)",
            seed.canonical_post_id,
            tag_id
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

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/roadmap?tenant_slug={}&tag=security",
                seed.tenant_slug
            ))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        assert!(!items.is_empty());
        assert!(items.iter().any(|item| {
            item.get("id").and_then(|v| v.as_str()) == Some(&seed.canonical_post_id.to_string())
        }));
    }

    #[actix_web::test]
    async fn roadmap_hides_private_items_from_anonymous_users() {
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

        let anonymous_request = test::TestRequest::get()
            .uri(&format!("/api/roadmap?tenant_slug={}", seed.tenant_slug))
            .to_request();
        let anonymous_response = test::call_service(&app, anonymous_request).await;
        assert_eq!(anonymous_response.status(), StatusCode::OK);
        let anonymous_json = read_json(anonymous_response).await;
        let anonymous_items = anonymous_json.as_array().unwrap();
        assert!(anonymous_items
            .iter()
            .all(|item| item.get("id").and_then(|v| v.as_str())
                != Some(seed.private_post_id.to_string().as_str())));

        let token = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );
        let member_request = test::TestRequest::get()
            .uri(&format!("/api/roadmap?tenant_slug={}", seed.tenant_slug))
            .insert_header(("Authorization", token))
            .to_request();
        let member_response = test::call_service(&app, member_request).await;
        assert_eq!(member_response.status(), StatusCode::OK);
        let member_json = read_json(member_response).await;
        let member_items = member_json.as_array().unwrap();
        let private_post_id = seed.private_post_id.to_string();
        assert!(member_items
            .iter()
            .any(|item| item.get("id").and_then(|v| v.as_str()) == Some(private_post_id.as_str())));
    }

    #[actix_web::test]
    async fn roadmap_excludes_duplicates_by_default() {
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

        let request = test::TestRequest::get()
            .uri(&format!("/api/roadmap?tenant_slug={}", seed.tenant_slug))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        let duplicate_id = seed.duplicate_post_id.to_string();
        assert!(!items
            .iter()
            .any(|item| item.get("id").and_then(|v| v.as_str()) == Some(duplicate_id.as_str())));
    }

    #[actix_web::test]
    async fn cross_tenant_user_cannot_access_post_by_id() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tenant_b_id = uuid::Uuid::new_v4();
        let tenant_b_slug = format!("tenant-b-{}", uuid::Uuid::new_v4().simple());
        let user_b_id = uuid::Uuid::new_v4();
        let user_b_subject = format!("user-b-{}", uuid::Uuid::new_v4().simple());
        let board_b_id = uuid::Uuid::new_v4();
        let post_b_id = uuid::Uuid::new_v4();

        sqlx::query!(
            "INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Tenant B')",
            tenant_b_id,
            tenant_b_slug
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO users (id, rooiam_subject, email, display_name) VALUES ($1, $2, $3, 'User B')",
            user_b_id,
            user_b_subject,
            "user-b@example.com"
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO memberships (tenant_id, user_id, role) VALUES ($1, $2, 'member')",
            tenant_b_id,
            user_b_id
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO boards (id, tenant_id, slug, name, board_type, is_private) VALUES ($1, $2, 'board-b', 'Board B', 'feature-requests', FALSE)",
            board_b_id,
            tenant_b_id,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO posts (id, tenant_id, board_id, user_id, title, body, status) VALUES ($1, $2, $3, $4, 'Post B', 'body', 'planned')",
            post_b_id,
            tenant_b_id,
            board_b_id,
            user_b_id,
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

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/posts/{}?tenant_slug={}",
                post_b_id, seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn roadmap_excludes_under_review_and_declined() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET status = 'declined' WHERE id = $1",
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

        let request = test::TestRequest::get()
            .uri(&format!("/api/roadmap?tenant_slug={}", seed.tenant_slug))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        for item in items {
            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
            assert!(
                matches!(status, "planned" | "in_progress" | "done"),
                "status '{status}' should not appear in public roadmap"
            );
        }
    }

    #[actix_web::test]
    async fn long_post_title_is_rejected() {
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
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );

        let long_title = "x".repeat(1001);
        let request = test::TestRequest::post()
            .uri(&format!(
                "/api/posts?tenant_slug={}&board_slug={}",
                seed.tenant_slug, seed.board_slug
            ))
            .insert_header(("Authorization", token))
            .set_json(json!({
                "title": long_title,
                "body": "valid body"
            }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_ne!(response.status(), StatusCode::CREATED);
        assert!(response.status().is_client_error());
    }

    #[actix_web::test]
    async fn roadmap_grouped_returns_three_groups() {
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

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/roadmap/grouped?tenant_slug={}",
                seed.tenant_slug
            ))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert!(body.get("planned").and_then(|v| v.as_array()).is_some());
        assert!(body.get("in_progress").and_then(|v| v.as_array()).is_some());
        assert!(body.get("done").and_then(|v| v.as_array()).is_some());
    }
}
