use actix_web::{http::StatusCode, test, web, App};
use serde_json::json;
use uuid::Uuid;

use crate::{
    db,
    http::test_support::{
        bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
    },
    startup,
};

use super::delivery::{enqueue, process_one, EmailRequest, QueueResult};

#[actix_web::test]
async fn unicode_email_subjects_keep_valid_utf8_within_limit() {
    let title = "🐺".repeat(100);
    let short = super::delivery::prefix_utf8(&title, 180);
    assert_eq!(short.len(), 180);
    assert_eq!(short.chars().count(), 45);
}

async fn setup() -> (crate::config::Settings, db::DbPool, Uuid, Uuid, String) {
    std::env::set_var(
        "HOWLLO_EMAIL_CONFIG_KEY",
        "d3b8269fb40911727081c064032604a4b3f05d6dfd935158a70278477648915b",
    );
    let settings = test_settings();
    let pool = db::establish_connection(&settings.database_url)
        .await
        .unwrap();
    reset_db(&pool).await;
    sqlx::query("DELETE FROM admin_credentials")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM admin_auth_rate_limit")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM email_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM email_messages")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM email_suppression")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE email_platform_settings SET smtp_host='',from_email='',status='disabled',tested_at=NULL,tested_version=NULL,failure_count=0,config_version=config_version+1")
        .execute(&pool).await.unwrap();
    let seed = seed_basic_tenant(&pool).await;
    let tenant_id: Uuid = sqlx::query_scalar("SELECT id FROM tenants WHERE slug=$1")
        .bind(&seed.tenant_slug)
        .fetch_one(&pool)
        .await
        .unwrap();
    (
        settings,
        pool,
        tenant_id,
        seed.member_user_id,
        seed.tenant_slug,
    )
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
async fn platform_disabled_roles_config_and_mailhog_delivery() {
    let _guard = lock_test_db().await;
    let (settings, pool, tenant_id, member_id, slug) = setup().await;
    let app = app!(pool, settings);
    let disabled = enqueue(
        &pool,
        EmailRequest {
            tenant_id: Some(tenant_id),
            user_id: None,
            recipient: "someone@example.test",
            kind: "system",
            category: "invitation",
            subject: "Invitation",
            body: "Join",
            dedupe_key: None,
            delay_seconds: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(disabled, QueueResult::Skipped("platform_disabled"));

    let member = bearer_for(
        "member-sub",
        "member@example.com",
        "Member",
        &settings.rooiam_jwt_secret,
    );
    let forbidden = test::TestRequest::get()
        .uri("/api/admin/platform/email")
        .insert_header(("Authorization", member))
        .to_request();
    assert_eq!(
        test::call_service(&app, forbidden).await.status(),
        StatusCode::FORBIDDEN
    );
    let anonymous = test::TestRequest::get()
        .uri("/api/admin/platform/email")
        .to_request();
    assert_eq!(
        test::call_service(&app, anonymous).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let setup_req = test::TestRequest::post()
        .uri("/api/admin/auth/setup")
        .set_json(
            json!({"bootstrap_key":"test-bootstrap-key","password":"very-strong-admin-password"}),
        )
        .to_request();
    let setup_res = test::call_service(&app, setup_req).await;
    let setup_res = if setup_res.status() == StatusCode::FORBIDDEN {
        let login = test::TestRequest::post()
            .uri("/api/admin/auth/login")
            .set_json(json!({"password":"very-strong-admin-password"}))
            .to_request();
        test::call_service(&app, login).await
    } else {
        setup_res
    };
    assert_eq!(setup_res.status(), StatusCode::OK);
    let admin_token = format!(
        "Bearer {}",
        read_json(setup_res).await["access_token"].as_str().unwrap()
    );
    let staff_block = test::TestRequest::post()
        .uri("/api/admin/platform/email/suppression")
        .insert_header((
            "Authorization",
            bearer_for(
                "member-sub",
                "member@example.com",
                "Member",
                &settings.rooiam_jwt_secret,
            ),
        ))
        .set_json(json!({"recipient":"blocked@example.test","reason":"complaint"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, staff_block).await.status(),
        StatusCode::FORBIDDEN
    );
    let block = test::TestRequest::post()
        .uri("/api/admin/platform/email/suppression")
        .insert_header(("Authorization", admin_token.clone()))
        .set_json(json!({"recipient":"blocked@example.test","reason":"complaint"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, block).await.status(),
        StatusCode::NO_CONTENT
    );
    let list = test::TestRequest::get()
        .uri("/api/admin/platform/email/suppression")
        .insert_header(("Authorization", admin_token.clone()))
        .to_request();
    assert_eq!(
        read_json(test::call_service(&app, list).await).await[0]["recipient"],
        "blocked@example.test"
    );
    let unblock = test::TestRequest::delete()
        .uri("/api/admin/platform/email/suppression?recipient=blocked%40example.test")
        .insert_header(("Authorization", admin_token.clone()))
        .to_request();
    assert_eq!(
        test::call_service(&app, unblock).await.status(),
        StatusCode::NO_CONTENT
    );
    let hidden = test::TestRequest::get()
        .uri(&format!("/api/admin/tenant-email?tenant_slug={slug}"))
        .insert_header(("Authorization", admin_token.clone()))
        .to_request();
    assert_eq!(
        test::call_service(&app, hidden).await.status(),
        StatusCode::NOT_FOUND
    );
    let save = test::TestRequest::put().uri("/api/admin/platform/email")
        .insert_header(("Authorization", admin_token.clone()))
        .set_json(json!({"smtp_host":"127.0.0.1","smtp_port":1025,"smtp_tls_mode":"none",
          "smtp_username":"","smtp_password":"test-secret","from_email":"mail@howllo.test","from_name":"Howllo",
          "reply_to":"","monthly_limit":100,"tenant_daily_limit":10,"tenant_monthly_limit":30,
          "max_broadcast_recipients":10,"broadcasts_per_day":2,"broadcasts_per_week":5})).to_request();
    let saved = test::call_service(&app, save).await;
    assert_eq!(saved.status(), StatusCode::OK);
    let saved_json = read_json(saved).await;
    assert_eq!(saved_json["status"], "configured");
    assert!(saved_json.get("smtp_password").is_none());
    assert_eq!(saved_json["smtp_password_configured"], true);
    let stored: String =
        sqlx::query_scalar("SELECT smtp_password_ciphertext FROM email_platform_settings")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!stored.contains("test-secret"));
    let premature = test::TestRequest::post()
        .uri("/api/admin/platform/email/enabled")
        .insert_header(("Authorization", admin_token.clone()))
        .set_json(json!({"enabled":true}))
        .to_request();
    assert_eq!(
        test::call_service(&app, premature).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    if std::env::var("HOWLLO_TEST_MAILHOG").as_deref() == Ok("1") {
        let recipient = format!("howllo-{}@example.test", Uuid::new_v4());
        let test_req = test::TestRequest::post()
            .uri("/api/admin/platform/email/test")
            .insert_header(("Authorization", admin_token.clone()))
            .set_json(json!({"recipient":recipient}))
            .to_request();
        let tested = test::call_service(&app, test_req).await;
        assert_eq!(tested.status(), StatusCode::OK);
        assert_eq!(read_json(tested).await["status"], "tested");
        let enable = test::TestRequest::post()
            .uri("/api/admin/platform/email/enabled")
            .insert_header(("Authorization", admin_token.clone()))
            .set_json(json!({"enabled":true}))
            .to_request();
        assert_eq!(
            read_json(test::call_service(&app, enable).await).await["status"],
            "enabled"
        );
        assert!(matches!(
            enqueue(
                &pool,
                EmailRequest {
                    tenant_id: Some(tenant_id),
                    user_id: None,
                    recipient: &recipient,
                    kind: "system",
                    category: "invitation",
                    subject: "Invitation from Howllo",
                    body: "Join the workspace",
                    dedupe_key: Some(format!("test:invite:{recipient}")),
                    delay_seconds: 0
                }
            )
            .await
            .unwrap(),
            QueueResult::Queued(_)
        ));
        assert!(process_one(&pool).await.unwrap());
        let status: String =
            sqlx::query_scalar("SELECT status FROM email_messages WHERE recipient=$1")
                .bind(&recipient)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "sent");
        let mailbox = reqwest::get("http://127.0.0.1:8025/api/v2/messages?limit=1000")
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            mailbox.contains(&recipient),
            "MailHog did not capture the test and invitation emails"
        );
        let changed = test::TestRequest::put()
            .uri("/api/admin/platform/email")
            .insert_header(("Authorization", admin_token.clone()))
            .set_json(
                json!({"smtp_host":"127.0.0.1","smtp_port":1025,"smtp_tls_mode":"none",
              "smtp_username":"","from_email":"mail@howllo.test","from_name":"Updated Howllo",
              "reply_to":"","monthly_limit":100,"tenant_daily_limit":10,"tenant_monthly_limit":30,
              "max_broadcast_recipients":10,"broadcasts_per_day":2,"broadcasts_per_week":5}),
            )
            .to_request();
        let changed = test::call_service(&app, changed).await;
        assert_eq!(changed.status(), StatusCode::OK);
        assert_eq!(read_json(changed).await["status"], "configured");
        let premature = test::TestRequest::post()
            .uri("/api/admin/platform/email/enabled")
            .insert_header(("Authorization", admin_token.clone()))
            .set_json(json!({"enabled":true}))
            .to_request();
        assert_eq!(
            test::call_service(&app, premature).await.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let _ = member_id;
}

#[actix_web::test]
async fn quota_dedupe_opt_out_and_system_priority() {
    let _guard = lock_test_db().await;
    let (_, pool, tenant_id, member_id, _) = setup().await;
    sqlx::query("UPDATE email_platform_settings SET status='enabled',monthly_limit=1,tenant_daily_limit=1,tenant_monthly_limit=1")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_email_settings(tenant_id,enabled) VALUES($1,TRUE)")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .unwrap();
    let address = "member-email@example.test";
    sqlx::query("INSERT INTO user_verified_emails(user_id,email) VALUES($1,$2)")
        .bind(member_id)
        .bind(address)
        .execute(&pool)
        .await
        .unwrap();
    let make = |key: &str| EmailRequest {
        tenant_id: Some(tenant_id),
        user_id: Some(member_id),
        recipient: address,
        kind: "notification",
        category: "reply",
        subject: "New reply",
        body: "Someone replied",
        dedupe_key: Some(key.to_owned()),
        delay_seconds: 300,
    };
    assert!(matches!(
        enqueue(&pool, make("thread:1")).await.unwrap(),
        QueueResult::Queued(_)
    ));
    assert_eq!(
        enqueue(&pool, make("thread:1")).await.unwrap(),
        QueueResult::Duplicate
    );
    let count: i32 =
        sqlx::query_scalar("SELECT event_count FROM email_messages WHERE dedupe_key='thread:1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        enqueue(&pool, make("thread:2")).await.unwrap(),
        QueueResult::Skipped("tenant_quota")
    );
    sqlx::query(
        "INSERT INTO user_email_preferences(tenant_id,user_id,replies_enabled) VALUES($1,$2,FALSE)",
    )
    .bind(tenant_id)
    .bind(member_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        enqueue(&pool, make("thread:3")).await.unwrap(),
        QueueResult::Skipped("user_opt_out")
    );
    assert!(matches!(
        enqueue(
            &pool,
            EmailRequest {
                tenant_id: None,
                user_id: Some(member_id),
                recipient: address,
                kind: "system",
                category: "password_reset",
                subject: "Reset",
                body: "Use this code",
                dedupe_key: Some("reset:1".into()),
                delay_seconds: 0
            }
        )
        .await
        .unwrap(),
        QueueResult::Queued(_)
    ));
    sqlx::query("INSERT INTO email_suppression(recipient,reason) VALUES($1,'manual test')")
        .bind(address)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        enqueue(
            &pool,
            EmailRequest {
                tenant_id: None,
                user_id: Some(member_id),
                recipient: address,
                kind: "system",
                category: "password_reset",
                subject: "Reset again",
                body: "Code",
                dedupe_key: Some("reset:2".into()),
                delay_seconds: 0
            }
        )
        .await
        .unwrap(),
        QueueResult::Skipped("suppressed")
    );
}

#[actix_web::test]
async fn verified_email_reset_is_private_one_time_and_revokes_sessions() {
    let _guard = lock_test_db().await;
    let (settings, pool, _tenant_id, member_id, _) = setup().await;
    sqlx::query("UPDATE email_platform_settings SET status='enabled'")
        .execute(&pool)
        .await
        .unwrap();
    let subject: String = sqlx::query_scalar("SELECT rooiam_subject FROM users WHERE id=$1")
        .bind(member_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let bearer = bearer_for(
        &subject,
        "member@example.com",
        "Member",
        &settings.rooiam_jwt_secret,
    );
    let app = app!(pool, settings);
    let old_password = "old-password-12345";
    sqlx::query("INSERT INTO local_credentials(user_id,username,password_hash) VALUES($1,'email_member',$2)")
        .bind(member_id).bind(crate::auth::local_user::password_hash(old_password).unwrap())
        .execute(&pool).await.unwrap();
    let address = format!("verified-{}@example.test", Uuid::new_v4());
    let verify = test::TestRequest::post()
        .uri("/api/me/email-verification/request")
        .insert_header(("Authorization", bearer.clone()))
        .set_json(json!({"email": address}))
        .to_request();
    assert_eq!(
        test::call_service(&app, verify).await.status(),
        StatusCode::ACCEPTED
    );
    let cipher: String = sqlx::query_scalar("SELECT body FROM email_messages WHERE category='email_verification' ORDER BY created_at DESC LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let plain = super::decrypt(&cipher).unwrap();
    let code = plain.split("\n\n").nth(1).unwrap().trim();
    let confirm = test::TestRequest::post()
        .uri("/api/me/email-verification/confirm")
        .insert_header(("Authorization", bearer.clone()))
        .set_json(json!({"token": code}))
        .to_request();
    assert_eq!(
        test::call_service(&app, confirm).await.status(),
        StatusCode::OK
    );
    let replay = test::TestRequest::post()
        .uri("/api/me/email-verification/confirm")
        .insert_header(("Authorization", bearer))
        .set_json(json!({"token": code}))
        .to_request();
    assert_eq!(
        test::call_service(&app, replay).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let unknown = test::TestRequest::post()
        .uri("/api/auth/local/request-email-reset")
        .set_json(json!({"email":"unknown@example.test"}))
        .to_request();
    let known = test::TestRequest::post()
        .uri("/api/auth/local/request-email-reset")
        .set_json(json!({"email":address}))
        .to_request();
    assert_eq!(
        test::call_service(&app, unknown).await.status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(
        test::call_service(&app, known).await.status(),
        StatusCode::ACCEPTED
    );
    let resets: i64 =
        sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE category='password_reset'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(resets, 1);
    let cipher: String =
        sqlx::query_scalar("SELECT body FROM email_messages WHERE category='password_reset'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let plain = super::decrypt(&cipher).unwrap();
    let reset_code = plain.split("\n\n").nth(1).unwrap().trim();
    sqlx::query("INSERT INTO account_sessions(user_id,token_hash,expires_at) VALUES($1,$2,NOW()+INTERVAL '1 day')")
        .bind(member_id).bind(format!("test-session-{}", Uuid::new_v4())).execute(&pool).await.unwrap();
    let reset = test::TestRequest::post()
        .uri("/api/auth/local/confirm-email-reset")
        .set_json(json!({"token":reset_code,"new_password":"new-password-12345"}))
        .to_request();
    let reset_response = test::call_service(&app, reset).await;
    assert_eq!(reset_response.status(), StatusCode::OK);
    let new_recovery = read_json(reset_response).await["recovery_code"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(new_recovery.starts_with("howllo_rc_"));
    let revoked: bool = sqlx::query_scalar(
        "SELECT bool_and(revoked_at IS NOT NULL) FROM account_sessions WHERE user_id=$1",
    )
    .bind(member_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(revoked);
    let replay = test::TestRequest::post()
        .uri("/api/auth/local/confirm-email-reset")
        .set_json(json!({"token":reset_code,"new_password":"attacker-password-12345"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, replay).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let expired_code = "known-expired-reset-token";
    sqlx::query("INSERT INTO local_email_reset_tokens(user_id,token_hash,expires_at) VALUES($1,$2,NOW()-INTERVAL '1 minute')")
        .bind(member_id).bind(super::message_hash(expired_code)).execute(&pool).await.unwrap();
    let expired = test::TestRequest::post()
        .uri("/api/auth/local/confirm-email-reset")
        .set_json(json!({"token":expired_code,"new_password":"attacker-password-12345"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, expired).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let old = test::TestRequest::post()
        .uri("/api/auth/local/login")
        .set_json(json!({"username":"email_member","password":old_password}))
        .to_request();
    assert_eq!(
        test::call_service(&app, old).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let fresh = test::TestRequest::post()
        .uri("/api/auth/local/login")
        .set_json(json!({"username":"email_member","password":"new-password-12345"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, fresh).await.status(),
        StatusCode::OK
    );
}

#[actix_web::test]
async fn tenant_broadcast_requires_opt_in_and_daily_limit_and_digest_dedupes() {
    let _guard = lock_test_db().await;
    let (settings, pool, tenant_id, member_id, slug) = setup().await;
    sqlx::query("UPDATE email_platform_settings SET status='enabled',broadcasts_per_day=1,max_broadcast_recipients=2,monthly_limit=20")
        .execute(&pool).await.unwrap();
    let admin_subject: String = sqlx::query_scalar("SELECT u.rooiam_subject FROM users u JOIN memberships m ON m.user_id=u.id WHERE m.tenant_id=$1 AND m.role='admin' LIMIT 1")
        .bind(tenant_id).fetch_one(&pool).await.unwrap();
    let staff = bearer_for(
        &admin_subject,
        "admin@example.com",
        "Admin",
        &settings.rooiam_jwt_secret,
    );
    let app = app!(pool, settings);
    let settings_req = test::TestRequest::get()
        .uri(&format!("/api/admin/tenant-email?tenant_slug={slug}"))
        .insert_header(("Authorization", staff.clone()))
        .to_request();
    let current = read_json(test::call_service(&app, settings_req).await).await;
    assert_eq!(current["enabled"], false);
    let foreign = test::TestRequest::get()
        .uri(&format!("/api/admin/tenant-email?tenant_slug={slug}"))
        .insert_header((
            "Authorization",
            bearer_for(
                "foreign-sub",
                "foreign@example.test",
                "Foreign",
                &settings.rooiam_jwt_secret,
            ),
        ))
        .to_request();
    assert_eq!(
        test::call_service(&app, foreign).await.status(),
        StatusCode::FORBIDDEN
    );
    let too_high = test::TestRequest::put()
        .uri(&format!("/api/admin/tenant-email?tenant_slug={slug}"))
        .insert_header(("Authorization", staff.clone()))
        .set_json(json!({"enabled":true,"reply_notifications":true,"important_updates":true,
          "digest_enabled":true,"broadcast_enabled":true,"daily_limit":100001,"monthly_limit":null}))
        .to_request();
    assert_eq!(
        test::call_service(&app, too_high).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let enable = test::TestRequest::put()
        .uri(&format!("/api/admin/tenant-email?tenant_slug={slug}"))
        .insert_header(("Authorization", staff.clone()))
        .set_json(json!({"enabled":true,
          "reply_notifications":true,"important_updates":true,"digest_enabled":true,
          "broadcast_enabled":true,"daily_limit":null,"monthly_limit":null}))
        .to_request();
    assert_eq!(
        test::call_service(&app, enable).await.status(),
        StatusCode::OK
    );
    let no_opt_in = test::TestRequest::post()
        .uri(&format!(
            "/api/admin/tenant-email/broadcast?tenant_slug={slug}"
        ))
        .insert_header(("Authorization", staff.clone()))
        .set_json(json!({"subject":"News","body":"New product release"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, no_opt_in).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let address = "member-broadcast@example.test";
    sqlx::query("UPDATE memberships SET public_participant=TRUE WHERE tenant_id=$1 AND user_id=$2")
        .bind(tenant_id)
        .bind(member_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_verified_emails(user_id,email) VALUES($1,$2)")
        .bind(member_id)
        .bind(address)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_email_preferences(tenant_id,user_id,broadcast_enabled,digest_enabled) VALUES($1,$2,TRUE,TRUE)")
        .bind(tenant_id).bind(member_id).execute(&pool).await.unwrap();
    let broadcast = || {
        test::TestRequest::post()
            .uri(&format!(
                "/api/admin/tenant-email/broadcast?tenant_slug={slug}"
            ))
            .insert_header(("Authorization", staff.clone()))
            .set_json(json!({"subject":"News","body":"New product release"}))
            .to_request()
    };
    let first = test::call_service(&app, broadcast()).await;
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    assert_eq!(read_json(first).await["queued"], 1);
    assert_eq!(
        test::call_service(&app, broadcast()).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    sqlx::query("UPDATE email_platform_settings SET broadcasts_per_day=10,broadcasts_per_week=1")
        .execute(&pool)
        .await
        .unwrap();
    let weekly = test::call_service(&app, broadcast()).await;
    assert_eq!(weekly.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        read_json(weekly).await["error"]["message"],
        "Weekly announcement limit reached."
    );
    sqlx::query("UPDATE user_email_preferences SET broadcast_enabled=FALSE WHERE tenant_id=$1 AND user_id=$2")
        .bind(tenant_id).bind(member_id).execute(&pool).await.unwrap();
    assert!(process_one(&pool).await.unwrap());
    let status: String =
        sqlx::query_scalar("SELECT status FROM email_messages WHERE kind='broadcast'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "cancelled");
    sqlx::query("INSERT INTO notifications(tenant_id,user_id,event_type,title,body) VALUES($1,$2,'status_changed','Update','A post changed')")
        .bind(tenant_id).bind(member_id).execute(&pool).await.unwrap();
    assert_eq!(
        super::delivery::enqueue_daily_digests(&pool).await.unwrap(),
        1
    );
    assert_eq!(
        super::delivery::enqueue_daily_digests(&pool).await.unwrap(),
        0
    );
    let digests: i64 =
        sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE kind='digest'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(digests, 1);
    sqlx::query("INSERT INTO workspace_restrictions(tenant_id,user_id,kind,reason) VALUES($1,$2,'banned','test ban')")
        .bind(tenant_id).bind(member_id).execute(&pool).await.unwrap();
    assert!(process_one(&pool).await.unwrap());
    let digest_status: String =
        sqlx::query_scalar("SELECT status FROM email_messages WHERE kind='digest'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(digest_status, "cancelled");
}

#[actix_web::test]
async fn smtp_failure_retries_then_pauses_platform() {
    let _guard = lock_test_db().await;
    let (_, pool, tenant_id, _, _) = setup().await;
    sqlx::query("UPDATE email_platform_settings SET status='enabled',smtp_host='127.0.0.1',smtp_port=1,smtp_tls_mode='none',smtp_username='',smtp_password_ciphertext=NULL,from_email='mail@howllo.test'")
        .execute(&pool).await.unwrap();
    let queued = enqueue(
        &pool,
        EmailRequest {
            tenant_id: Some(tenant_id),
            user_id: None,
            recipient: "retry@example.test",
            kind: "system",
            category: "invitation",
            subject: "Invitation",
            body: "Please join",
            dedupe_key: None,
            delay_seconds: 0,
        },
    )
    .await
    .unwrap();
    let QueueResult::Queued(id) = queued else {
        panic!("expected queued message")
    };
    for _ in 0..5 {
        assert!(process_one(&pool).await.unwrap());
        sqlx::query("UPDATE email_messages SET next_attempt_at=NOW() WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let state: (String, i32) =
        sqlx::query_as("SELECT status,failure_count FROM email_platform_settings")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, ("paused".into(), 5));
    let message: (String, i32) =
        sqlx::query_as("SELECT status,attempts FROM email_messages WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(message, ("failed".into(), 5));
    assert!(!process_one(&pool).await.unwrap());
}

#[actix_web::test]
async fn invitation_and_reset_request_limits_survive_new_handlers() {
    let _guard = lock_test_db().await;
    let (settings, pool, tenant_id, _, slug) = setup().await;
    let actor: Uuid = sqlx::query_scalar(
        "SELECT user_id FROM memberships WHERE tenant_id=$1 AND role='admin' LIMIT 1",
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO invitation_email_limits(tenant_id,actor_user_id,attempts) VALUES($1,$2,20)",
    )
    .bind(tenant_id)
    .bind(actor)
    .execute(&pool)
    .await
    .unwrap();
    let blocked = crate::services::invitation_service::create_invitation(
        &pool,
        &slug,
        actor,
        "new-staff@example.test",
        "moderator",
    )
    .await;
    assert!(matches!(
        blocked,
        Err(crate::errors::AppError::TooManyRequests(_))
    ));
    let invitations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM workspace_invitations WHERE tenant_id=$1")
            .bind(tenant_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(invitations, 0);

    sqlx::query("UPDATE email_platform_settings SET status='enabled'")
        .execute(&pool)
        .await
        .unwrap();
    let app = app!(pool, settings);
    for _ in 0..3 {
        let req = test::TestRequest::post()
            .uri("/api/auth/local/request-email-reset")
            .set_json(json!({"email":"unknown-limited@example.test"}))
            .to_request();
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::ACCEPTED
        );
    }
    let blocked = test::TestRequest::post()
        .uri("/api/auth/local/request-email-reset")
        .set_json(json!({"email":"unknown-limited@example.test"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, blocked).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}
