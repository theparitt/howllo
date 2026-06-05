use actix_web::{delete, get, post, web, HttpResponse, Responder};

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::services::subscription_service;

#[post("/api/posts/{post_id}/follow")]
pub async fn follow_post(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    let dto = subscription_service::follow_post(pool.get_ref(), post_id, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(dto))
}

#[delete("/api/posts/{post_id}/follow")]
pub async fn unfollow_post(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    let dto = subscription_service::unfollow_post(pool.get_ref(), post_id, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(dto))
}

#[get("/api/posts/{post_id}/follow")]
pub async fn get_follow_state(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    let dto = subscription_service::get_follow_state(pool.get_ref(), post_id, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(dto))
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};

    use crate::db;
    use crate::http::test_support::{
        bearer_for, lock_test_db, reset_db, seed_basic_tenant, test_settings,
    };
    use crate::startup;

    #[actix_web::test]
    async fn cannot_follow_hidden_post() {
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
            .uri(&format!("/api/posts/{}/follow", seed.canonical_post_id))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn locked_post_allows_follow() {
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
            .uri(&format!("/api/posts/{}/follow", seed.canonical_post_id))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
