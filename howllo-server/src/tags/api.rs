use actix_web::{delete, get, patch, post, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::dto::{CreateTagRequest, UpdateTagRequest};
use crate::errors::AppError;
use crate::services::tag_service;

#[get("/api/admin/tags")]
pub async fn list_admin_tags(
    pool: web::Data<DbPool>,
    query: web::Query<std::collections::HashMap<String, String>>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let slug = query
        .get("tenant_slug")
        .filter(|slug| !slug.trim().is_empty())
        .ok_or_else(|| AppError::Validation("tenant_slug is required".into()))?;
    let tenant =
        crate::repositories::membership_repository::resolve_tenant_id(pool.get_ref(), slug).await?;
    crate::auth::require_permission(
        pool.get_ref(),
        tenant,
        auth.0.id,
        crate::domain::permission::Permission::ManageTags,
    )
    .await?;
    let tags = crate::repositories::tag_repository::list_tags_for_tenant(pool.get_ref(), tenant)
        .await
        .map_err(|error| {
            tracing::error!(%error, "could not list admin tags");
            AppError::InternalServerError
        })?;
    Ok(HttpResponse::Ok().json(tags))
}

#[get("/api/tags")]
pub async fn list_tags(
    pool: web::Data<DbPool>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<impl Responder, AppError> {
    let tenant_slug = query
        .get("tenant_slug")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Validation("tenant_slug is required".to_string()))?;

    let tags = tag_service::list_tags(pool.get_ref(), tenant_slug).await?;
    Ok(HttpResponse::Ok().json(tags))
}

#[post("/api/admin/tags")]
pub async fn create_tag(
    pool: web::Data<DbPool>,
    body: web::Json<CreateTagRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    body.validate()?;
    let tag = tag_service::create_tag(
        pool.get_ref(),
        &body.tenant_slug,
        body.slug.trim(),
        body.name.trim(),
        body.color.as_deref(),
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Created().json(tag))
}

#[patch("/api/admin/tags/{tag_id}")]
pub async fn update_tag(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    body: web::Json<UpdateTagRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    body.validate()?;
    let tag_id = path.into_inner();
    let tag = tag_service::update_tag(
        pool.get_ref(),
        tag_id,
        body.name.trim(),
        body.color.as_deref(),
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(tag))
}

#[delete("/api/admin/tags/{tag_id}")]
pub async fn delete_tag(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tag_id = path.into_inner();
    tag_service::delete_tag(pool.get_ref(), tag_id, auth.0.id).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[post("/api/admin/posts/{post_id}/tags/{tag_id}")]
pub async fn attach_tag_to_post(
    pool: web::Data<DbPool>,
    path: web::Path<(uuid::Uuid, uuid::Uuid)>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let (post_id, tag_id) = path.into_inner();
    tag_service::attach_tag_to_post(pool.get_ref(), post_id, tag_id, auth.0.id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[delete("/api/admin/posts/{post_id}/tags/{tag_id}")]
pub async fn detach_tag_from_post(
    pool: web::Data<DbPool>,
    path: web::Path<(uuid::Uuid, uuid::Uuid)>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let (post_id, tag_id) = path.into_inner();
    tag_service::detach_tag_from_post(pool.get_ref(), post_id, tag_id, auth.0.id).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize)]
pub struct TagSummaryQuery {
    pub tenant_slug: String,
}

#[get("/api/admin/tags/summary")]
pub async fn get_tag_summary(
    pool: web::Data<DbPool>,
    query: web::Query<TagSummaryQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    use crate::auth::require_permission;
    use crate::domain::permission::Permission;

    let tenant_id = crate::repositories::membership_repository::resolve_tenant_id(
        pool.get_ref(),
        &query.tenant_slug,
    )
    .await?;

    require_permission(pool.get_ref(), tenant_id, auth.0.id, Permission::ManageTags).await?;

    let items = crate::repositories::tag_repository::get_tag_summary(pool.get_ref(), tenant_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_id = %tenant_id, "error fetching tag summary");
            AppError::InternalServerError
        })?;

    Ok(HttpResponse::Ok().json(items))
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
    async fn admin_can_create_and_list_tags() {
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
            .uri("/api/admin/tags")
            .insert_header(("Authorization", token))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "slug": "ux",
                "name": "UX",
                "color": "#333333"
            }))
            .to_request();
        let create_response = test::call_service(&app, create_request).await;
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let list_request = test::TestRequest::get()
            .uri(&format!("/api/tags?tenant_slug={}", seed.tenant_slug))
            .to_request();
        let list_response = test::call_service(&app, list_request).await;
        assert_eq!(list_response.status(), StatusCode::OK);

        let body = read_json(list_response).await;
        let items = body.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].get("slug").and_then(|v| v.as_str()), Some("ux"));
    }

    #[actix_web::test]
    async fn tag_summary_counts_are_correct() {
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

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/tags/summary?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert!(body.as_array().is_some());
    }

    #[actix_web::test]
    async fn cross_tenant_cannot_attach_tag() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tenant_b_id = uuid::Uuid::new_v4();
        let tag_b_id = uuid::Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO tenants (id, slug, name) VALUES ($1, 'tenant-b-2', 'B')",
            tenant_b_id
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO tags (id, tenant_id, slug, name, color) VALUES ($1, $2, 'b-tag', 'B Tag', '#000')",
            tag_b_id,
            tenant_b_id,
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

        let request = test::TestRequest::post()
            .uri(&format!(
                "/api/admin/posts/{}/tags/{tag_b_id}",
                seed.canonical_post_id
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert!(response.status().is_client_error());
    }
}
