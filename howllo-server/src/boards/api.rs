use actix_web::{delete, get, patch, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::auth::{require_moderator, AuthenticatedUser};
use crate::db::DbPool;
use crate::dto::{CreateBoardRequest, UpdateBoardRequest};
use crate::errors::AppError;
use crate::repositories::board_repository;
use crate::services::board_service;

#[derive(Deserialize)]
pub struct BoardListQuery {
    pub tenant_slug: String,
}

#[derive(Deserialize)]
pub struct BoardDetailQuery {
    pub tenant_slug: String,
}

#[derive(Deserialize)]
pub struct AdminBoardListQuery {
    pub tenant_slug: String,
}

#[get("/api/boards")]
pub async fn list_boards(
    pool: web::Data<DbPool>,
    query: web::Query<BoardListQuery>,
) -> Result<impl Responder, AppError> {
    let boards = board_service::list_boards(pool.get_ref(), &query.tenant_slug).await?;
    Ok(HttpResponse::Ok().json(boards))
}

#[get("/api/boards/{board_slug}")]
pub async fn get_board_detail(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    query: web::Query<BoardDetailQuery>,
) -> Result<impl Responder, AppError> {
    let board_slug = path.into_inner();
    let board =
        board_service::get_board_detail(&req, pool.get_ref(), &query.tenant_slug, &board_slug)
            .await?;
    Ok(HttpResponse::Ok().json(board))
}

#[get("/api/admin/boards")]
pub async fn list_admin_boards(
    pool: web::Data<DbPool>,
    query: web::Query<AdminBoardListQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let boards =
        board_service::list_admin_boards(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(boards))
}

#[get("/api/admin/boards/count")]
pub async fn count_staff_boards(
    pool: web::Data<DbPool>,
    query: web::Query<AdminBoardListQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = board_repository::get_tenant_id_by_slug(pool.get_ref(), &query.tenant_slug)
        .await
        .map_err(|error| {
            tracing::error!(%error, "could not resolve workspace for board count");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;
    require_moderator(pool.get_ref(), tenant_id, auth.0.id).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM boards WHERE tenant_id = $1")
        .bind(tenant_id)
        .fetch_one(pool.get_ref())
        .await
        .map_err(|error| {
            tracing::error!(%error, "could not count workspace boards");
            AppError::InternalServerError
        })?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "count": count })))
}

#[post("/api/admin/boards")]
pub async fn create_board(
    pool: web::Data<DbPool>,
    body: web::Json<CreateBoardRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    body.validate()?;
    let board = board_service::create_board(
        pool.get_ref(),
        &body.tenant_slug,
        body.slug.trim(),
        body.name.trim(),
        body.description.as_deref().map(str::trim),
        body.board_type.trim(),
        body.intro_text.as_deref().map(str::trim),
        body.allow_votes,
        body.allow_comments,
        body.is_private,
        body.icon_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
        body.background_color
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
        body.header_image_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
        body.background_image_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
        body.dashboard_sections.clone(),
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Created().json(board))
}

#[patch("/api/admin/boards/{board_id}")]
pub async fn update_board(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdateBoardRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    body.validate()?;
    let board_id = path.into_inner();
    let board = board_service::update_board(
        pool.get_ref(),
        board_id,
        body.name.trim(),
        body.description.as_deref().map(str::trim),
        body.board_type.trim(),
        body.intro_text.as_deref().map(str::trim),
        body.allow_votes,
        body.allow_comments,
        body.is_private,
        body.icon_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
        body.background_color
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
        body.header_image_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
        body.background_image_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
        body.dashboard_sections.clone(),
        body.is_enabled,
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(board))
}

#[delete("/api/admin/boards/{board_id}")]
pub async fn delete_board(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let board_id = path.into_inner();
    board_service::delete_board(pool.get_ref(), board_id, auth.0.id).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize)]
pub struct BoardSummaryQuery {
    pub tenant_slug: String,
}

#[get("/api/admin/boards/{board_id}/summary")]
pub async fn get_board_summary(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    _query: web::Query<BoardSummaryQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let board_id = path.into_inner();
    let board = crate::repositories::board_repository::get_board_tenant(pool.get_ref(), board_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, board_id = %board_id, "error getting board for summary");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    crate::auth::require_permission(
        pool.get_ref(),
        board,
        auth.0.id,
        crate::domain::permission::Permission::ManageBoards,
    )
    .await?;

    let summary =
        crate::repositories::board_repository::get_board_summary(pool.get_ref(), board_id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, board_id = %board_id, "error fetching board summary");
                AppError::InternalServerError
            })?
            .ok_or(AppError::NotFound)?;

    Ok(HttpResponse::Ok().json(crate::dto::BoardSummaryDto {
        board_id,
        board_slug: summary.slug,
        board_name: summary.name,
        total_posts: summary.total_posts,
        posts_by_status: summary.posts_by_status,
        total_votes: summary.total_votes,
        total_comments: summary.total_comments,
        delete_posts_count: summary.delete_posts_count,
        delete_comments_count: summary.delete_comments_count,
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
    async fn board_count_is_visible_to_staff_but_not_public_members() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url).await.unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;
        let empty_tenant_id = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, 'empty-workspace', 'Empty')")
            .bind(empty_tenant_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO memberships (tenant_id, user_id, role) VALUES ($1, $2, 'moderator')")
            .bind(empty_tenant_id).bind(seed.moderator_user_id).execute(&pool).await.unwrap();
        let app = test::init_service(App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(settings.clone()))
            .configure(startup::configure)).await;
        let moderator = bearer_for(&seed.moderator_subject, "moderator@example.com", "Moderator", &settings.rooiam_jwt_secret);
        let member = bearer_for(&seed.member_subject, "member@example.com", "Member", &settings.rooiam_jwt_secret);
        let empty = test::call_service(&app, test::TestRequest::get()
            .uri("/api/admin/boards/count?tenant_slug=empty-workspace")
            .insert_header(("Authorization", moderator.clone())).to_request()).await;
        assert_eq!(empty.status(), StatusCode::OK);
        assert_eq!(read_json(empty).await["count"], 0);
        let existing = test::call_service(&app, test::TestRequest::get()
            .uri(&format!("/api/admin/boards/count?tenant_slug={}", seed.tenant_slug))
            .insert_header(("Authorization", moderator)).to_request()).await;
        assert_eq!(existing.status(), StatusCode::OK);
        assert_eq!(read_json(existing).await["count"], 2);
        let denied = test::call_service(&app, test::TestRequest::get()
            .uri(&format!("/api/admin/boards/count?tenant_slug={}", seed.tenant_slug))
            .insert_header(("Authorization", member)).to_request()).await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn announcement_board_enforces_staff_posts_and_disabled_interactions() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url).await.unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;
        sqlx::query("UPDATE posts SET created_at = now() - interval '2 days'")
            .execute(&pool).await.unwrap();

        let app = test::init_service(App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(settings.clone()))
            .app_data(web::Data::new(crate::realtime::Hub::new()))
            .configure(startup::configure)).await;
        let member = bearer_for(&seed.member_subject, "member@example.com", "Member", &settings.rooiam_jwt_secret);
        let admin = bearer_for(&seed.admin_subject, "admin@example.com", "Admin", &settings.rooiam_jwt_secret);
        let board_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM boards WHERE slug = $1")
            .bind(&seed.board_slug).fetch_one(&pool).await.unwrap();
        let settings_request = test::TestRequest::patch().uri(&format!("/api/admin/boards/{board_id}"))
            .insert_header(("Authorization", admin.clone()))
            .set_json(json!({"name": "Updates", "board_type": "announcements", "intro_text": "News from our team", "allow_votes": false, "allow_comments": false, "is_private": false}))
            .to_request();
        let settings_response = test::call_service(&app, settings_request).await;
        assert_eq!(settings_response.status(), StatusCode::OK);
        let board_settings = read_json(settings_response).await;
        assert_eq!(board_settings["intro_text"], "News from our team");
        assert_eq!(board_settings["allow_votes"], false);
        assert_eq!(board_settings["allow_comments"], false);
        let post_path = format!("/api/boards/{}/posts", seed.board_slug);

        let member_post = test::TestRequest::post().uri(&post_path)
            .insert_header(("Authorization", member.clone()))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "title": "Visitor update", "body": "Visitors must not publish here."}))
            .to_request();
        assert_eq!(test::call_service(&app, member_post).await.status(), StatusCode::FORBIDDEN);

        let staff_post = test::TestRequest::post().uri(&post_path)
            .insert_header(("Authorization", admin))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "title": "Team update", "body": "Available now."}))
            .to_request();
        assert_eq!(test::call_service(&app, staff_post).await.status(), StatusCode::CREATED);

        let vote = test::TestRequest::post().uri(&format!("/api/posts/{}/vote", seed.canonical_post_id))
            .insert_header(("Authorization", member.clone())).to_request();
        assert_eq!(test::call_service(&app, vote).await.status(), StatusCode::FORBIDDEN);
        let comment = test::TestRequest::post().uri(&format!("/api/posts/{}/comments", seed.canonical_post_id))
            .insert_header(("Authorization", member))
            .set_json(json!({"body": "A response"})).to_request();
        assert_eq!(test::call_service(&app, comment).await.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn private_board_requires_membership() {
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
            .uri(&format!(
                "/api/boards/{}?tenant_slug={}",
                seed.private_board_slug, seed.tenant_slug
            ))
            .to_request();
        let anonymous_response = test::call_service(&app, anonymous_request).await;
        assert_eq!(anonymous_response.status(), StatusCode::FORBIDDEN);

        let token = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );
        let member_request = test::TestRequest::get()
            .uri(&format!(
                "/api/boards/{}?tenant_slug={}",
                seed.private_board_slug, seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .to_request();
        let member_response = test::call_service(&app, member_request).await;
        assert_eq!(member_response.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn admin_can_create_list_and_update_boards() {
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
            .uri("/api/admin/boards")
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "slug": "release-notes",
                "name": "Release Notes",
                "description": "Track announcements and shipped work",
                "board_type": "changelog",
                "is_private": true,
                "background_color": "#fff1ea"
            }))
            .to_request();
        let create_response = test::call_service(&app, create_request).await;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created_body = read_json(create_response).await;
        assert_eq!(created_body["is_enabled"], false);
        assert!(created_body["first_enabled_at"].is_null());
        assert_eq!(
            created_body
                .get("background_color")
                .and_then(|value| value.as_str()),
            Some("#fff1ea")
        );
        let board_id = created_body
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap();

        let list_request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/boards?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .to_request();
        let list_response = test::call_service(&app, list_request).await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = read_json(list_response).await;
        assert!(list_body.as_array().unwrap().len() >= 3);

        let update_request = test::TestRequest::patch()
            .uri(&format!("/api/admin/boards/{board_id}"))
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({
                "name": "Releases",
                "description": "Product release communication",
                "board_type": "changelog",
                "is_private": false,
                "background_color": "#e8f4ff"
            }))
            .to_request();
        let update_response = test::call_service(&app, update_request).await;
        assert_eq!(update_response.status(), StatusCode::OK);
        let update_body = read_json(update_response).await;
        assert_eq!(
            update_body
                .get("background_color")
                .and_then(|value| value.as_str()),
            Some("#e8f4ff")
        );
        for enabled in [true, false, true] {
            let request = test::TestRequest::patch()
                .uri(&format!("/api/admin/boards/{board_id}"))
                .insert_header(("Authorization", token.clone()))
                .set_json(json!({"name": "Releases", "description": "Product release communication", "board_type": "changelog", "is_private": false, "is_enabled": enabled}))
                .to_request();
            let response = test::call_service(&app, request).await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = read_json(response).await;
            assert_eq!(body["is_enabled"], enabled);
            assert!(body["first_enabled_at"].is_string());

            let public_request = test::TestRequest::get()
                .uri(&format!(
                    "/api/boards/release-notes?tenant_slug={}",
                    seed.tenant_slug
                ))
                .to_request();
            assert_eq!(
                test::call_service(&app, public_request).await.status(),
                if enabled {
                    StatusCode::OK
                } else {
                    StatusCode::NOT_FOUND
                }
            );
        }
    }

    #[actix_web::test]
    async fn moderator_cannot_create_board() {
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
            .uri("/api/admin/boards")
            .insert_header(("Authorization", token))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "slug": "ops",
                "name": "Ops",
                "description": "Moderator should not create boards",
                "board_type": "internal",
                "is_private": true
            }))
            .to_request();
        let create_response = test::call_service(&app, create_request).await;
        assert_eq!(create_response.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn board_summary_counts_are_correct() {
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

        let board_row = sqlx::query!(
            "SELECT b.id FROM boards b JOIN posts p ON p.board_id = b.id WHERE p.id = $1",
            seed.canonical_post_id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let board_id = board_row.id.to_string();
        let tenant_id: uuid::Uuid =
            sqlx::query_scalar("SELECT tenant_id FROM boards WHERE id = $1")
                .bind(board_row.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let asset_root =
            std::env::temp_dir().join(format!("howllo-board-delete-{}", uuid::Uuid::new_v4()));
        let asset_key = format!("workspaces/{tenant_id}/uploads/unique.png");
        let asset_file = asset_root.join(&asset_key);
        std::fs::create_dir_all(asset_file.parent().unwrap()).unwrap();
        std::fs::write(&asset_file, b"image").unwrap();
        for (key, value) in [
            ("storage_backend", "local".to_string()),
            (
                "storage_local_path",
                asset_root.to_string_lossy().to_string(),
            ),
            ("storage_public_base_url", "https://assets.test".to_string()),
        ] {
            sqlx::query("INSERT INTO system_settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value")
                .bind(key).bind(value).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO workspace_assets (tenant_id, relative_path, size_bytes) VALUES ($1, $2, 5)")
            .bind(tenant_id).bind(&asset_key).execute(&pool).await.unwrap();
        sqlx::query("UPDATE posts SET attachments = jsonb_build_array($1::text) WHERE id = $2")
            .bind(format!("https://assets.test/{asset_key}"))
            .bind(seed.canonical_post_id)
            .execute(&pool)
            .await
            .unwrap();
        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/boards/{board_id}/summary?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert!(
            body.get("total_posts")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                > 0
        );
        assert!(body
            .get("posts_by_status")
            .and_then(|v| v.as_object())
            .is_some());
        assert!(
            body["delete_posts_count"].as_i64().unwrap() >= body["total_posts"].as_i64().unwrap()
        );
        assert!(
            body["delete_comments_count"].as_i64().unwrap()
                >= body["total_comments"].as_i64().unwrap()
        );

        let delete_request = test::TestRequest::delete()
            .uri(&format!("/api/admin/boards/{board_id}"))
            .insert_header(("Authorization", token))
            .to_request();
        assert_eq!(
            test::call_service(&app, delete_request).await.status(),
            StatusCode::NO_CONTENT
        );
        let remaining_posts: i64 =
            sqlx::query_scalar("SELECT count(*) FROM posts WHERE board_id = $1")
                .bind(board_row.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining_posts, 0);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let remaining: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM workspace_assets WHERE tenant_id = $1 AND relative_path = $2",
                )
                .bind(tenant_id)
                .bind(&asset_key)
                .fetch_one(&pool)
                .await
                .unwrap();
                if !asset_file.exists() && remaining == 0 { break; }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("deleted board assets should be cleaned up");
        let remaining_assets: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM workspace_assets WHERE tenant_id = $1 AND relative_path = $2",
        )
        .bind(tenant_id)
        .bind(&asset_key)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(remaining_assets, 0);
        std::fs::remove_dir_all(asset_root).unwrap();
    }

    #[actix_web::test]
    async fn listing_boards_does_not_create_default_boards() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;

        let tenant_id = uuid::Uuid::new_v4();
        let tenant_slug = format!("tenant-{}", uuid::Uuid::new_v4().simple());
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Tenant')")
            .bind(tenant_id)
            .bind(&tenant_slug)
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
            .uri(&format!("/api/boards?tenant_slug={tenant_slug}"))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let boards = body.as_array().unwrap();
        assert!(boards.is_empty());
    }

    #[actix_web::test]
    async fn deleting_default_board_disables_future_bootstrap() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;

        let tenant_id = uuid::Uuid::new_v4();
        let tenant_slug = format!("tenant-{}", uuid::Uuid::new_v4().simple());
        let admin_user_id = uuid::Uuid::new_v4();
        let admin_subject = format!("admin-{}", uuid::Uuid::new_v4().simple());

        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Tenant')")
            .bind(tenant_id)
            .bind(&tenant_slug)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO boards (tenant_id, slug, name, board_type, is_default) VALUES ($1, 'general', 'General', 'general', TRUE)")
            .bind(tenant_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO users (id, rooiam_subject, email, display_name) VALUES ($1, $2, 'admin@example.com', 'Admin')",
        )
        .bind(admin_user_id)
        .bind(&admin_subject)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO memberships (tenant_id, user_id, role) VALUES ($1, $2, 'admin')")
            .bind(tenant_id)
            .bind(admin_user_id)
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
            &admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let list_request = test::TestRequest::get()
            .uri(&format!("/api/admin/boards?tenant_slug={tenant_slug}"))
            .insert_header(("Authorization", token.clone()))
            .to_request();
        let list_response = test::call_service(&app, list_request).await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = read_json(list_response).await;
        let board_id = list_body[0]
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap()
            .to_string();

        let delete_request = test::TestRequest::delete()
            .uri(&format!("/api/admin/boards/{board_id}"))
            .insert_header(("Authorization", token.clone()))
            .to_request();
        let delete_response = test::call_service(&app, delete_request).await;
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let public_list_request = test::TestRequest::get()
            .uri(&format!("/api/boards?tenant_slug={tenant_slug}"))
            .to_request();
        let public_list_response = test::call_service(&app, public_list_request).await;
        assert_eq!(public_list_response.status(), StatusCode::OK);
        let public_list_body = read_json(public_list_response).await;
        let remaining = public_list_body.as_array().unwrap();
        assert_eq!(remaining.len(), 0);
        let remaining_ids = remaining
            .iter()
            .map(|board| board.get("id").and_then(|value| value.as_str()).unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(!remaining_ids.contains(board_id.as_str()));
    }
}
