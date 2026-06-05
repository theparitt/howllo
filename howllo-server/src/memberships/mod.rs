use crate::db::DbPool;
use crate::domain::role::Role;
use crate::dto::{CreateMemberRequest, MembershipItemDto, UpdateMemberRoleRequest};
use crate::errors::AppError;
use crate::services::membership_service;
use actix_web::{delete, get, patch, post, web, HttpResponse, Responder};

use crate::auth::AuthenticatedUser;

pub async fn check_membership(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<Role, ()> {
    sqlx::query!(
        "SELECT role FROM memberships WHERE tenant_id = $1 AND user_id = $2",
        tenant_id,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, tenant_id = %tenant_id, user_id = %user_id, "error checking membership");
    })
    .and_then(|opt| opt.ok_or(()))
    .and_then(|row| Role::parse(&row.role).map_err(|error| {
        tracing::error!(error = %error, role = row.role, tenant_id = %tenant_id, user_id = %user_id, "invalid membership role in database");
    }))
}

pub async fn is_admin(
    pool: &DbPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<bool, ()> {
    check_membership(pool, tenant_id, user_id)
        .await
        .map(|role| matches!(role, Role::Owner | Role::Admin))
}

#[derive(Debug, serde::Deserialize)]
pub struct MemberListQuery {
    pub tenant_slug: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct RemoveMemberQuery {
    pub tenant_slug: String,
}

#[get("/api/admin/members")]
pub async fn list_members(
    pool: web::Data<DbPool>,
    query: web::Query<MemberListQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let items: Vec<MembershipItemDto> =
        membership_service::list_members(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(items))
}

#[post("/api/admin/members")]
pub async fn create_member(
    pool: web::Data<DbPool>,
    body: web::Json<CreateMemberRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let item = membership_service::create_member(pool.get_ref(), &body, auth.0.id).await?;
    Ok(HttpResponse::Created().json(item))
}

#[patch("/api/admin/members/{user_id}/role")]
pub async fn update_member_role(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<MemberListQuery>,
    body: web::Json<UpdateMemberRoleRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let item = membership_service::update_member_role(
        pool.get_ref(),
        &query.tenant_slug,
        path.into_inner(),
        &body,
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(item))
}

#[delete("/api/admin/members/{user_id}")]
pub async fn remove_member(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<RemoveMemberQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    membership_service::remove_member(
        pool.get_ref(),
        &query.tenant_slug,
        path.into_inner(),
        auth.0.id,
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
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
    async fn admin_can_manage_members_and_audit_role_changes() {
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

        let token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let create_request = test::TestRequest::post()
            .uri("/api/admin/members")
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({
                "tenant_slug": seed.tenant_slug,
                "email": "new-member@example.com",
                "display_name": "New Member",
                "role": "member"
            }))
            .to_request();
        let create_response = test::call_service(&app, create_request).await;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = read_json(create_response).await;
        let user_id = created
            .get("user_id")
            .and_then(|value| value.as_str())
            .unwrap();

        let patch_request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/members/{user_id}/role?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({ "role": "moderator" }))
            .to_request();
        let patch_response = test::call_service(&app, patch_request).await;
        assert_eq!(patch_response.status(), StatusCode::OK);

        let list_request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/members?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .to_request();
        let list_response = test::call_service(&app, list_request).await;
        assert_eq!(list_response.status(), StatusCode::OK);

        let audit_request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/audit-logs?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .to_request();
        let audit_response = test::call_service(&app, audit_request).await;
        assert_eq!(audit_response.status(), StatusCode::OK);
        let audit = read_json(audit_response).await;
        assert_eq!(
            audit
                .pointer("/items/0/action")
                .and_then(|value| value.as_str()),
            Some(crate::audit::MEMBER_ROLE_CHANGED)
        );

        let delete_request = test::TestRequest::delete()
            .uri(&format!(
                "/api/admin/members/{user_id}?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let delete_response = test::call_service(&app, delete_request).await;
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);
    }
}
