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
    let created = invitation_service::create_invitation(
        pool.get_ref(),
        &body.tenant_slug,
        auth.0.id,
        &body.email,
        &body.role,
    )
    .await?;

    Ok(HttpResponse::Created()
        .insert_header(("Cache-Control", "no-store"))
        .json(serde_json::json!({
            "id": created.invitation.id,
            "email": created.invitation.email,
            "role": created.invitation.role,
            "status": created.invitation.status,
            "redemption_code": created.redemption_code,
        })))
}

#[derive(Debug, Deserialize)]
pub struct RedeemInvitationRequest {
    pub code: String,
}

#[post("/api/me/invitations/redeem")]
pub async fn redeem_invitation(
    pool: web::Data<DbPool>,
    body: web::Json<RedeemInvitationRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    invitation_service::redeem_invitation(pool.get_ref(), auth.0.id, &body.code).await?;
    Ok(HttpResponse::Ok().finish())
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
    use uuid::Uuid;

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
    async fn standalone_staff_code_is_single_use_and_scoped() {
        let _guard = lock_test_db().await;
        let (mut settings, pool, seed) = setup().await;
        settings.rooiam_legacy_hs256_enabled = false;
        settings.rooiam_hosted_userinfo_url = None;
        let app = app!(pool, settings);

        let register = |name: &str| {
            test::TestRequest::post()
                .uri("/api/auth/local/register")
                .set_json(json!({"username":name,"password":"a-strong-local-password"}))
                .to_request()
        };
        let owner = read_json(test::call_service(&app, register("standalone_owner")).await).await;
        let staff = read_json(test::call_service(&app, register("standalone_staff")).await).await;
        let stranger =
            read_json(test::call_service(&app, register("standalone_other")).await).await;
        let owner_token = format!("Bearer {}", owner["access_token"].as_str().unwrap());
        let staff_token = format!("Bearer {}", staff["access_token"].as_str().unwrap());
        let stranger_token = format!("Bearer {}", stranger["access_token"].as_str().unwrap());
        let tenant_id: Uuid = sqlx::query_scalar("SELECT id FROM tenants WHERE slug=$1")
            .bind(&seed.tenant_slug)
            .fetch_one(&pool)
            .await
            .unwrap();
        let owner_id: Uuid = sqlx::query_scalar(
            "SELECT user_id FROM local_credentials WHERE username='standalone_owner'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let staff_id: Uuid = sqlx::query_scalar(
            "SELECT user_id FROM local_credentials WHERE username='standalone_staff'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO memberships (tenant_id,user_id,role) VALUES ($1,$2,'owner')")
            .bind(tenant_id)
            .bind(owner_id)
            .execute(&pool)
            .await
            .unwrap();

        let created = read_json(test::call_service(&app, test::TestRequest::post()
            .uri("/api/admin/invitations")
            .insert_header(("Authorization", owner_token.clone()))
            .set_json(json!({"tenant_slug":seed.tenant_slug,"email":"staff@example.com","role":"moderator"}))
            .to_request()).await).await;
        let code = created["redemption_code"].as_str().unwrap();
        let invite_id = created["id"].as_str().unwrap();
        assert!(code.starts_with("howllo_inv_"));
        let stored_hash: String = sqlx::query_scalar(
            "SELECT redemption_code_hash FROM workspace_invitations WHERE id=$1",
        )
        .bind(Uuid::parse_str(invite_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(stored_hash, code);

        let unbound = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/me/invitations/{invite_id}/accept"))
                .insert_header(("Authorization", stranger_token.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(unbound.status(), StatusCode::NOT_FOUND);
        let wrong = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/me/invitations/redeem")
                .insert_header(("Authorization", stranger_token.clone()))
                .set_json(json!({"code":"howllo_inv_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}))
                .to_request(),
        )
        .await;
        assert_eq!(wrong.status(), StatusCode::NOT_FOUND);

        let accepted = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/me/invitations/redeem")
                .insert_header(("Authorization", staff_token.clone()))
                .set_json(json!({"code":code}))
                .to_request(),
        )
        .await;
        assert_eq!(accepted.status(), StatusCode::OK);
        let role: String =
            sqlx::query_scalar("SELECT role FROM memberships WHERE tenant_id=$1 AND user_id=$2")
                .bind(tenant_id)
                .bind(staff_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(role, "moderator");
        let staff_session = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/workspace-session")
                .insert_header(("Authorization", staff_token))
                .set_json(json!({"tenant_slug":seed.tenant_slug}))
                .to_request(),
        )
        .await;
        assert_eq!(staff_session.status(), StatusCode::CREATED);
        let session_token = read_json(staff_session).await["session_token"]
            .as_str()
            .unwrap()
            .to_string();
        let role_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/me/workspace-role?tenant_slug={}",
                    seed.tenant_slug
                ))
                .insert_header(("Authorization", format!("Bearer {session_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(role_response.status(), StatusCode::OK);
        let replay = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/me/invitations/redeem")
                .insert_header(("Authorization", stranger_token.clone()))
                .set_json(json!({"code":code}))
                .to_request(),
        )
        .await;
        assert_eq!(replay.status(), StatusCode::NOT_FOUND);
        let stranger_id: Uuid = sqlx::query_scalar(
            "SELECT user_id FROM local_credentials WHERE username='standalone_other'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let stranger_membership: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM memberships WHERE tenant_id=$1 AND user_id=$2",
        )
        .bind(tenant_id)
        .bind(stranger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stranger_membership, 0);

        let expired = read_json(test::call_service(&app, test::TestRequest::post()
            .uri("/api/admin/invitations")
            .insert_header(("Authorization", owner_token.clone()))
            .set_json(json!({"tenant_slug":seed.tenant_slug,"email":"expired@example.com","role":"admin"}))
            .to_request()).await).await;
        sqlx::query(
            "UPDATE workspace_invitations SET expires_at=NOW()-INTERVAL '1 second' WHERE id=$1",
        )
        .bind(Uuid::parse_str(expired["id"].as_str().unwrap()).unwrap())
        .execute(&pool)
        .await
        .unwrap();
        let expired_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/me/invitations/redeem")
                .insert_header(("Authorization", stranger_token.clone()))
                .set_json(json!({"code":expired["redemption_code"]}))
                .to_request(),
        )
        .await;
        assert_eq!(expired_response.status(), StatusCode::NOT_FOUND);

        let withdrawn = read_json(test::call_service(&app, test::TestRequest::post()
            .uri("/api/admin/invitations")
            .insert_header(("Authorization", owner_token.clone()))
            .set_json(json!({"tenant_slug":seed.tenant_slug,"email":"withdrawn@example.com","role":"admin"}))
            .to_request()).await).await;
        let withdrawn_id = withdrawn["id"].as_str().unwrap();
        let withdraw = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!(
                    "/api/admin/invitations/{withdrawn_id}/withdraw?tenant_slug={}",
                    seed.tenant_slug
                ))
                .insert_header(("Authorization", owner_token))
                .to_request(),
        )
        .await;
        assert_eq!(withdraw.status(), StatusCode::NO_CONTENT);
        let withdrawn_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/me/invitations/redeem")
                .insert_header(("Authorization", stranger_token))
                .set_json(json!({"code":withdrawn["redemption_code"]}))
                .to_request(),
        )
        .await;
        assert_eq!(withdrawn_response.status(), StatusCode::NOT_FOUND);
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

        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workspace_invitations WHERE email = $1 AND status = 'pending'",
        )
        .bind(STAFF_EMAIL)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            pending, 1,
            "a duplicate or denied invite must not mutate state"
        );
    }

    #[actix_web::test]
    async fn accepted_invitation_cannot_be_replayed_or_rejected() {
        let _guard = lock_test_db().await;
        let (settings, pool, seed) = setup().await;
        let app = app!(pool, settings);
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let staff = bearer_for(
            STAFF_SUBJECT,
            STAFF_EMAIL,
            "New Staff",
            &settings.rooiam_jwt_secret,
        );

        let created = read_json(test::call_service(&app, test::TestRequest::post()
            .uri("/api/admin/invitations")
            .insert_header(("Authorization", admin))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "email": STAFF_EMAIL, "role": "moderator"}))
            .to_request()).await).await;
        let id = created["id"].as_str().unwrap();
        let invite_id = Uuid::parse_str(id).unwrap();

        let mine = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/me/invitations")
                .insert_header(("Authorization", staff.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(mine.status(), StatusCode::OK);
        let accept_uri = format!("/api/me/invitations/{id}/accept");
        let reject_uri = format!("/api/me/invitations/{id}/reject");
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&accept_uri)
                    .insert_header(("Authorization", staff.clone()))
                    .to_request()
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&accept_uri)
                    .insert_header(("Authorization", staff.clone()))
                    .to_request()
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&reject_uri)
                    .insert_header(("Authorization", staff))
                    .to_request()
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );

        let (status, membership_count, audit_count): (String, i64, i64) = sqlx::query_as(
            "SELECT i.status, (SELECT COUNT(*) FROM memberships WHERE tenant_id = i.tenant_id AND user_id = i.user_id), (SELECT COUNT(*) FROM audit_logs WHERE entity_id = i.id AND action = 'invitation_accepted') FROM workspace_invitations i WHERE i.id = $1",
        ).bind(invite_id).fetch_one(&pool).await.unwrap();
        assert_eq!(status, "accepted");
        assert_eq!(membership_count, 1);
        assert_eq!(audit_count, 1);
    }

    #[actix_web::test]
    async fn expired_invitation_cannot_be_accepted() {
        let _guard = lock_test_db().await;
        let (settings, pool, seed) = setup().await;
        let app = app!(pool, settings);
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let staff = bearer_for(
            STAFF_SUBJECT,
            STAFF_EMAIL,
            "New Staff",
            &settings.rooiam_jwt_secret,
        );

        let created = read_json(test::call_service(&app, test::TestRequest::post()
            .uri("/api/admin/invitations")
            .insert_header(("Authorization", admin))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "email": STAFF_EMAIL, "role": "admin"}))
            .to_request()).await).await;
        let invite_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
        let expiration: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT expires_at FROM workspace_invitations WHERE id = $1")
                .bind(invite_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            expiration.is_some(),
            "new invitations must have a finite lifetime"
        );
        let mine = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/me/invitations")
                .insert_header(("Authorization", staff.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(mine.status(), StatusCode::OK);
        sqlx::query("UPDATE workspace_invitations SET expires_at = NOW() - INTERVAL '1 second' WHERE id = $1")
            .bind(invite_id).execute(&pool).await.unwrap();
        let accept = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/me/invitations/{invite_id}/accept"))
                .insert_header(("Authorization", staff))
                .to_request(),
        )
        .await;
        assert_eq!(accept.status(), StatusCode::NOT_FOUND);
        let (status, membership_count): (String, i64) = sqlx::query_as(
            "SELECT i.status, (SELECT COUNT(*) FROM memberships WHERE tenant_id = i.tenant_id AND user_id = i.user_id) FROM workspace_invitations i WHERE i.id = $1",
        ).bind(invite_id).fetch_one(&pool).await.unwrap();
        assert_eq!(status, "expired");
        assert_eq!(membership_count, 0);
    }

    #[actix_web::test]
    async fn withdrawn_invitation_cannot_be_accepted() {
        let _guard = lock_test_db().await;
        let (settings, pool, seed) = setup().await;
        let app = app!(pool, settings);
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let staff = bearer_for(
            STAFF_SUBJECT,
            STAFF_EMAIL,
            "New Staff",
            &settings.rooiam_jwt_secret,
        );

        let created = read_json(test::call_service(&app, test::TestRequest::post()
            .uri("/api/admin/invitations")
            .insert_header(("Authorization", admin.clone()))
            .set_json(json!({"tenant_slug": seed.tenant_slug, "email": STAFF_EMAIL, "role": "admin"}))
            .to_request()).await).await;
        let invite_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
        let mine = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/me/invitations")
                .insert_header(("Authorization", staff.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(mine.status(), StatusCode::OK);
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&format!(
                        "/api/admin/invitations/{invite_id}/withdraw?tenant_slug={}",
                        seed.tenant_slug
                    ))
                    .insert_header(("Authorization", admin))
                    .to_request()
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&format!("/api/me/invitations/{invite_id}/accept"))
                    .insert_header(("Authorization", staff))
                    .to_request()
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let membership_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM memberships m JOIN workspace_invitations i ON i.tenant_id = m.tenant_id AND i.user_id = m.user_id WHERE i.id = $1",
        ).bind(invite_id).fetch_one(&pool).await.unwrap();
        assert_eq!(membership_count, 0);
    }

    #[actix_web::test]
    async fn rejected_invitation_does_not_grant_membership() {
        let _guard = lock_test_db().await;
        let (settings, pool, seed) = setup().await;
        let app = app!(pool, settings);
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let staff = bearer_for(
            STAFF_SUBJECT,
            STAFF_EMAIL,
            "New Staff",
            &settings.rooiam_jwt_secret,
        );

        let created = read_json(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/admin/invitations")
                    .insert_header(("Authorization", admin))
                    .set_json(json!({"tenant_slug": seed.tenant_slug, "email": STAFF_EMAIL, "role": "moderator"}))
                    .to_request(),
            )
            .await,
        )
        .await;
        let invite_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
        let mine = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/me/invitations")
                .insert_header(("Authorization", staff.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(mine.status(), StatusCode::OK);
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&format!("/api/me/invitations/{invite_id}/reject"))
                    .insert_header(("Authorization", staff.clone()))
                    .to_request(),
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&format!("/api/me/invitations/{invite_id}/accept"))
                    .insert_header(("Authorization", staff))
                    .to_request(),
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let (status, membership_count, audit_count): (String, i64, i64) = sqlx::query_as(
            "SELECT i.status, (SELECT COUNT(*) FROM memberships WHERE tenant_id = i.tenant_id AND user_id = i.user_id), (SELECT COUNT(*) FROM audit_logs WHERE entity_id = i.id AND action = 'invitation_rejected') FROM workspace_invitations i WHERE i.id = $1",
        )
        .bind(invite_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, "rejected");
        assert_eq!(membership_count, 0);
        assert_eq!(audit_count, 1);
    }

    #[actix_web::test]
    async fn revoked_members_permission_takes_effect_without_new_login() {
        let _guard = lock_test_db().await;
        let (settings, pool, seed) = setup().await;
        let app = app!(pool, settings);
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let invite = |email: &str| {
            test::TestRequest::post()
                .uri("/api/admin/invitations")
                .insert_header(("Authorization", admin.clone()))
                .set_json(
                    json!({"tenant_slug": seed.tenant_slug, "email": email, "role": "moderator"}),
                )
                .to_request()
        };
        assert_eq!(
            test::call_service(&app, invite("first@example.com"))
                .await
                .status(),
            StatusCode::CREATED
        );
        sqlx::query("UPDATE memberships SET role = 'member' WHERE tenant_id = (SELECT id FROM tenants WHERE slug = $1) AND user_id = $2")
            .bind(&seed.tenant_slug)
            .bind(seed.admin_user_id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            test::call_service(&app, invite("second@example.com"))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        let second_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workspace_invitations WHERE email = 'second@example.com'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(second_count, 0);
    }

    #[actix_web::test]
    async fn foreign_workspace_admin_cannot_withdraw_invitation() {
        let _guard = lock_test_db().await;
        let (settings, pool, seed) = setup().await;
        let app = app!(pool, settings);
        let original_admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let foreign_user_id = Uuid::new_v4();
        let foreign_subject = format!("foreign-admin-{}", Uuid::new_v4().simple());
        let foreign_admin = bearer_for(
            &foreign_subject,
            "foreign-admin@example.com",
            "Foreign Admin",
            &settings.rooiam_jwt_secret,
        );
        let foreign_tenant_id = Uuid::new_v4();
        let foreign_slug = format!("foreign-{}", Uuid::new_v4().simple());
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Foreign')")
            .bind(foreign_tenant_id)
            .bind(&foreign_slug)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id, rooiam_subject, email, display_name) VALUES ($1, $2, 'foreign-admin@example.com', 'Foreign Admin')")
            .bind(foreign_user_id)
            .bind(&foreign_subject)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (tenant_id, user_id, role) VALUES ($1, $2, 'admin')")
            .bind(foreign_tenant_id)
            .bind(foreign_user_id)
            .execute(&pool)
            .await
            .unwrap();

        let created = read_json(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/admin/invitations")
                    .insert_header(("Authorization", original_admin))
                    .set_json(json!({"tenant_slug": seed.tenant_slug, "email": STAFF_EMAIL, "role": "admin"}))
                    .to_request(),
            )
            .await,
        )
        .await;
        let invite_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
        sqlx::query("UPDATE workspace_invitations SET expires_at = NOW() - INTERVAL '1 second' WHERE id = $1")
            .bind(invite_id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&format!(
                        "/api/admin/invitations/{invite_id}/withdraw?tenant_slug={foreign_slug}"
                    ))
                    .insert_header(("Authorization", foreign_admin))
                    .to_request(),
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let status: String =
            sqlx::query_scalar("SELECT status FROM workspace_invitations WHERE id = $1")
                .bind(invite_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "pending");
    }
}
