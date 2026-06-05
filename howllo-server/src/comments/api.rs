use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::dto::CreateCommentRequest;
use crate::errors::AppError;
use crate::services::comment_service;

#[derive(Deserialize)]
pub struct CommentListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[post("/api/posts/{post_id}/comments")]
pub async fn create_comment(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<CreateCommentRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    let user_id = auth.0.id;
    body.validate()?;
    let settings = req
        .app_data::<web::Data<crate::config::Settings>>()
        .ok_or(AppError::InternalServerError)?;

    if body.body.chars().count() > settings.max_comment_body_chars {
        return Err(AppError::Validation(format!(
            "body must be {} characters or fewer",
            settings.max_comment_body_chars
        )));
    }

    let comment =
        comment_service::create_comment(pool.get_ref(), post_id, user_id, &body.body).await?;
    Ok(HttpResponse::Created().json(comment))
}

#[get("/api/posts/{post_id}/comments")]
pub async fn list_comments(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<CommentListQuery>,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    let comments =
        comment_service::list_comments(&req, pool.get_ref(), post_id, query.page, query.per_page)
            .await?;
    Ok(HttpResponse::Ok().json(comments))
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
    async fn cannot_create_comment_on_hidden_post() {
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
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );

        let request = test::TestRequest::post()
            .uri(&format!("/api/posts/{}/comments", seed.canonical_post_id))
            .insert_header(("Authorization", token))
            .set_json(json!({ "body": "hidden posts should reject comments" }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn comment_creation_is_rate_limited_when_enabled() {
        let mut settings = test_settings();
        settings.rate_limit_enabled = true;
        settings.public_write_rate_limit = 1;

        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let app = test::init_service(
            App::new()
                .wrap(crate::http::PublicWriteRateLimit::new())
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

        let request_a = test::TestRequest::post()
            .uri(&format!("/api/posts/{}/comments", seed.canonical_post_id))
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({ "body": "first comment" }))
            .to_request();
        let response_a = test::call_service(&app, request_a).await;
        assert_eq!(response_a.status(), StatusCode::CREATED);

        let request_b = test::TestRequest::post()
            .uri(&format!("/api/posts/{}/comments", seed.canonical_post_id))
            .insert_header(("Authorization", token))
            .set_json(json!({ "body": "second comment" }))
            .to_request();
        let response_b = test::try_call_service(&app, request_b).await;
        assert!(response_b.is_err());
    }

    #[actix_web::test]
    async fn hidden_comments_are_excluded_from_public_list() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let comment_id = uuid::Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO comments (id, post_id, user_id, body, is_hidden) VALUES ($1, $2, $3, $4, TRUE)",
            comment_id,
            seed.canonical_post_id,
            seed.member_user_id,
            "hidden comment"
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
            .uri(&format!("/api/posts/{}/comments", seed.canonical_post_id))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        let comment_id_str = comment_id.to_string();
        for item in items {
            assert_ne!(
                item.get("id").and_then(|v| v.as_str()),
                Some(comment_id_str.as_str())
            );
        }
    }

    #[actix_web::test]
    async fn comment_pagination_works() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        for i in 0..5 {
            let comment_id = uuid::Uuid::new_v4();
            sqlx::query!(
                "INSERT INTO comments (id, post_id, user_id, body) VALUES ($1, $2, $3, $4)",
                comment_id,
                seed.canonical_post_id,
                seed.member_user_id,
                format!("comment {i}")
            )
            .execute(&pool)
            .await
            .unwrap();
        }

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
                "/api/posts/{}/comments?page=1&per_page=2",
                seed.canonical_post_id
            ))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        assert!(items.len() <= 2);
    }

    #[actix_web::test]
    async fn long_comment_body_is_rejected() {
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

        let long_body = "x".repeat(settings.max_comment_body_chars + 1);
        let request = test::TestRequest::post()
            .uri(&format!("/api/posts/{}/comments", seed.canonical_post_id))
            .insert_header(("Authorization", token))
            .set_json(json!({ "body": long_body }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert!(response.status().is_client_error());
    }
}
