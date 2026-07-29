use actix_web::{delete, post, web, HttpResponse, Responder};

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::realtime::{Hub, RealtimeEvent};
use crate::repositories::post_repository;
use crate::services::vote_service;

#[post("/api/posts/{post_id}/vote")]
pub async fn add_vote(
    pool: web::Data<DbPool>,
    hub: web::Data<Hub>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    vote_service::add_vote(pool.get_ref(), post_id, auth.0.id).await?;

    if let Some(scope) = post_repository::find_post_access(pool.get_ref(), post_id, None)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, post_id = %post_id, "error loading vote realtime scope");
            AppError::InternalServerError
        })?
    {
        hub.broadcast(
            scope.tenant_id,
            RealtimeEvent {
                event_type: "post.vote_changed".to_string(),
                board_id: Some(scope.board_id),
                post_id: Some(post_id),
            },
        );
    }

    Ok(HttpResponse::Ok().finish())
}

#[delete("/api/posts/{post_id}/vote")]
pub async fn remove_vote(
    pool: web::Data<DbPool>,
    hub: web::Data<Hub>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    vote_service::remove_vote(pool.get_ref(), post_id, auth.0.id).await?;

    if let Some(scope) = post_repository::find_post_access(pool.get_ref(), post_id, None)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, post_id = %post_id, "error loading unvote realtime scope");
            AppError::InternalServerError
        })?
    {
        hub.broadcast(
            scope.tenant_id,
            RealtimeEvent {
                event_type: "post.vote_changed".to_string(),
                board_id: Some(scope.board_id),
                post_id: Some(post_id),
            },
        );
    }

    Ok(HttpResponse::Ok().finish())
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
    async fn cannot_vote_on_soft_deleted_post() {
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
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );

        let request = test::TestRequest::post()
            .uri(&format!("/api/posts/{}/vote", seed.canonical_post_id))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn locked_post_rejects_vote() {
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
            .uri(&format!("/api/posts/{}/vote", seed.canonical_post_id))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
