use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::auth::{maybe_authenticated_user, require_optional_api_token_scope};
use crate::db::DbPool;
use crate::errors::AppError;
use crate::repositories::search_repository;

#[derive(Debug, Deserialize)]
pub struct SearchPostsQuery {
    pub tenant_slug: String,
    pub q: String,
    pub sort: Option<String>,
    pub tag: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SuggestDuplicatesQuery {
    pub tenant_slug: String,
    pub q: String,
}

#[get("/api/search/posts")]
pub async fn search_posts(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    query: web::Query<SearchPostsQuery>,
) -> Result<impl Responder, AppError> {
    let _ = require_optional_api_token_scope(&req, "posts:read").await?;
    let user = maybe_authenticated_user(&req).await?;
    let items = search_repository::search_posts(
        pool.get_ref(),
        &query.tenant_slug,
        &query.q,
        query.sort.as_deref().unwrap_or("active"),
        None,
        query.tag.as_deref(),
        query.status.as_deref(),
        user.map(|u| u.id),
        20,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "error searching posts");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Ok().json(items))
}

#[get("/api/boards/{board_slug}/suggest-duplicates")]
pub async fn suggest_duplicates(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    query: web::Query<SuggestDuplicatesQuery>,
) -> Result<impl Responder, AppError> {
    let _ = require_optional_api_token_scope(&req, "posts:read").await?;
    let user = maybe_authenticated_user(&req).await?;
    let items = search_repository::search_posts(
        pool.get_ref(),
        &query.tenant_slug,
        &query.q,
        "most_voted",
        Some(&path.into_inner()),
        None,
        None,
        user.map(|u| u.id),
        5,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "error suggesting duplicates");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Ok().json(items))
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
    async fn search_finds_title_match_and_excludes_hidden_posts() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET title = 'Searchable roadmap item' WHERE id = $1",
            seed.canonical_post_id
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "UPDATE posts SET title = 'Searchable hidden item', is_hidden = TRUE WHERE id = $1",
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
            .uri(&format!(
                "/api/search/posts?tenant_slug={}&q=searchable",
                seed.tenant_slug
            ))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].get("title").and_then(|value| value.as_str()),
            Some("Searchable roadmap item")
        );
    }

    #[actix_web::test]
    async fn search_finds_body_match() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET body = 'unique-search-term-xyz' WHERE id = $1",
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

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/search/posts?tenant_slug={}&q=unique-search-term-xyz",
                seed.tenant_slug
            ))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        assert!(!items.is_empty());
    }

    #[actix_web::test]
    async fn search_excludes_deleted_posts() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET title = 'deleted searchable', deleted_at = NOW() WHERE id = $1",
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
            .uri(&format!(
                "/api/search/posts?tenant_slug={}&q=deleted+searchable",
                seed.tenant_slug
            ))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        assert!(items.is_empty());
    }

    #[actix_web::test]
    async fn search_excludes_private_posts_for_anonymous() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET title = 'private-searchable' WHERE id = $1",
            seed.private_post_id
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
                "/api/search/posts?tenant_slug={}&q=private-searchable",
                seed.tenant_slug
            ))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        assert!(items.is_empty());
    }

    #[actix_web::test]
    async fn search_includes_private_posts_for_member() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        sqlx::query!(
            "UPDATE posts SET title = 'member-private-search' WHERE id = $1",
            seed.private_post_id
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
                "/api/search/posts?tenant_slug={}&q=member-private-search",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        assert!(!items.is_empty());
    }
}
