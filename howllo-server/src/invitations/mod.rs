use actix_web::{get, post, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::services::invitation_service;

#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    pub tenant_slug: String,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct TenantScopeQuery {
    pub tenant_slug: String,
}

// ---------------------------------------------------------------- Owner side

#[post("/api/admin/invitations")]
pub async fn create_invitation(
    pool: web::Data<DbPool>,
    body: web::Json<CreateInvitationRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let invitation = invitation_service::create_invitation(
        pool.get_ref(),
        &body.tenant_slug,
        auth.0.id,
        &body.email,
        &body.role,
    )
    .await?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "id": invitation.id,
        "email": invitation.email,
        "role": invitation.role,
        "status": invitation.status,
    })))
}

#[get("/api/admin/invitations")]
pub async fn list_invitations(
    pool: web::Data<DbPool>,
    query: web::Query<TenantScopeQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let items =
        invitation_service::list_invitations(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(items))
}

#[post("/api/admin/invitations/{invitation_id}/withdraw")]
pub async fn withdraw_invitation(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<TenantScopeQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    invitation_service::withdraw_invitation(
        pool.get_ref(),
        &query.tenant_slug,
        auth.0.id,
        path.into_inner(),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

// --------------------------------------------------------------- Invitee side

#[get("/api/me/invitations")]
pub async fn list_my_invitations(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let items = invitation_service::list_my_invitations(pool.get_ref(), auth.0.id).await?;
    Ok(HttpResponse::Ok().json(items))
}

#[post("/api/me/invitations/{invitation_id}/accept")]
pub async fn accept_invitation(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    invitation_service::accept_invitation(pool.get_ref(), auth.0.id, path.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/api/me/invitations/{invitation_id}/reject")]
pub async fn reject_invitation(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    invitation_service::reject_invitation(pool.get_ref(), auth.0.id, path.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;

    use crate::db;
    use crate::http::test_support::{
        bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings, SeedData,
    };
    use crate::startup;

    const STAFF_EMAIL: &str = "newstaff@example.com";
    const STAFF_SUBJECT: &str = "newstaff-sub";

    async fn setup() -> (crate::config::Settings, crate::db::DbPool, SeedData) {
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;
        (settings, pool, seed)
    }

    macro_rules! app {
        ($pool:expr, $settings:expr) => {
            test::init_service(
                App::new()
                    .wrap(crate::http::RequestId)
                    .app_data(web::Data::new($pool.clone()))
                    .app_data(web::Data::new($settings.clone()))
                    .app_data(web::Data::new(crate::realtime::Hub::new()))
                    .configure(startup::configure),
            )
            .await
        };
    }

    #[actix_web::test]
    async fn owner_invites_staff_and_invitee_accepts() {
        let _guard = lock_test_db().await;
        let (settings, pool, seed) = setup().await;
        let app = app!(pool, settings);

        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        // Owner/admin invites staff by email.
        let create = test::TestRequest::post()
            .uri("/api/admin/invitations")
            .insert_header(("Authorization", admin.clone()))
            .set_json(json!({ "tenant_slug": seed.tenant_slug, "email": STAFF_EMAIL, "role": "moderator" }))
            .to_request();
        assert_eq!(
            test::call_service(&app, create).await.status(),
            StatusCode::CREATED
        );

        // Invitee signs into the workspace -> pending invite binds to their account.
        let staff = bearer_for(
            STAFF_SUBJECT,
            STAFF_EMAIL,
            "New Staff",
            &settings.rooiam_jwt_secret,
        );
        let session = test::TestRequest::post()
            .uri("/api/auth/workspace-session")
            .insert_header(("Authorization", staff.clone()))
            .set_json(json!({ "tenant_slug": seed.tenant_slug }))
            .to_request();
        assert_eq!(
            test::call_service(&app, session).await.status(),
            StatusCode::CREATED
        );

        // They see the pending invite and accept it.
        let mine = read_json(
            test::call_service(
                &app,
                test::TestRequest::get()
                    .uri("/api/me/invitations")
                    .insert_header(("Authorization", staff.clone()))
                    .to_request(),
            )
            .await,
        )
        .await;
        let items = mine.as_array().unwrap();
        assert_eq!(items.len(), 1);
        let invite_id = items[0].get("id").and_then(|v| v.as_str()).unwrap();

        let accept = test::TestRequest::post()
            .uri(&format!("/api/me/invitations/{invite_id}/accept"))
            .insert_header(("Authorization", staff))
            .to_request();
        assert_eq!(
            test::call_service(&app, accept).await.status(),
            StatusCode::OK
        );

        // Membership now exists with the invited role.
        let (role, public_participant): (String, bool) = sqlx::query_as(
            "SELECT m.role, m.public_participant FROM memberships m JOIN users u ON u.id = m.user_id WHERE u.rooiam_subject = $1",
        )
        .bind(STAFF_SUBJECT)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(role, "moderator");
        assert!(
            !public_participant,
            "accepted staff must not remain a public participant"
        );

        // The inviter got an "accepted" notification.
        let notifs = read_json(
            test::call_service(
                &app,
                test::TestRequest::get()
                    .uri("/api/notifications")
                    .insert_header(("Authorization", admin))
                    .to_request(),
            )
            .await,
        )
        .await;
        assert!(notifs
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.get("event_type").and_then(|v| v.as_str()) == Some("invitation_accepted")));
    }

    #[actix_web::test]
    async fn owner_can_withdraw_pending_invite() {
        let _guard = lock_test_db().await;
        let (settings, pool, seed) = setup().await;
        let app = app!(pool, settings);
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let created = read_json(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/admin/invitations")
                    .insert_header(("Authorization", admin.clone()))
                    .set_json(json!({ "tenant_slug": seed.tenant_slug, "email": STAFF_EMAIL, "role": "admin" }))
                    .to_request(),
            )
            .await,
        )
        .await;
        let id = created.get("id").and_then(|v| v.as_str()).unwrap();

        let withdraw = test::TestRequest::post()
            .uri(&format!(
                "/api/admin/invitations/{id}/withdraw?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", admin.clone()))
            .to_request();
        assert_eq!(
            test::call_service(&app, withdraw).await.status(),
            StatusCode::NO_CONTENT
        );

        // Withdrawing again fails (no longer pending).
        let again = test::TestRequest::post()
            .uri(&format!(
                "/api/admin/invitations/{id}/withdraw?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", admin))
            .to_request();
        assert_eq!(
            test::call_service(&app, again).await.status(),
            StatusCode::NOT_FOUND
        );
    }

    #[actix_web::test]
    async fn duplicate_pending_invite_is_rejected_and_members_cannot_invite() {
        let _guard = lock_test_db().await;
        let (settings, pool, seed) = setup().await;
        let app = app!(pool, settings);
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let make = |auth: String| {
            test::TestRequest::post()
                .uri("/api/admin/invitations")
                .insert_header(("Authorization", auth))
                .set_json(json!({ "tenant_slug": seed.tenant_slug, "email": STAFF_EMAIL, "role": "moderator" }))
                .to_request()
        };

        assert_eq!(
            test::call_service(&app, make(admin.clone())).await.status(),
            StatusCode::CREATED
        );
        // Second pending invite for the same email is rejected.
        assert_eq!(
            test::call_service(&app, make(admin)).await.status(),
            StatusCode::BAD_REQUEST
        );

        // A plain member cannot invite.
        let member = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );
        assert_eq!(
            test::call_service(&app, make(member)).await.status(),
            StatusCode::FORBIDDEN
        );
    }
}
