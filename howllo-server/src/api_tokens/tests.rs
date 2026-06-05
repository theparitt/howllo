use actix_web::{http::StatusCode, test, web, App};
use serde_json::json;

use crate::db;
use crate::http::test_support::{
    bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
};
use crate::startup;

#[actix_web::test]
async fn admin_can_create_list_and_revoke_api_token() {
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
        .uri("/api/admin/api-tokens")
        .insert_header(("Authorization", token.clone()))
        .set_json(json!({ "tenant_slug": seed.tenant_slug, "name": "CI" }))
        .to_request();
    let create_response = test::call_service(&app, create_request).await;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = read_json(create_response).await;
    assert!(created
        .get("token")
        .and_then(|v| v.as_str())
        .unwrap()
        .starts_with("howllo_"));
    let token_id = created
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    let list_request = test::TestRequest::get()
        .uri(&format!(
            "/api/admin/api-tokens?tenant_slug={}",
            seed.tenant_slug
        ))
        .insert_header(("Authorization", token.clone()))
        .to_request();
    let list_response = test::call_service(&app, list_request).await;
    assert_eq!(list_response.status(), StatusCode::OK);

    let revoke_request = test::TestRequest::patch()
        .uri(&format!("/api/admin/api-tokens/{token_id}/revoke"))
        .insert_header(("Authorization", token))
        .to_request();
    let revoke_response = test::call_service(&app, revoke_request).await;
    assert_eq!(revoke_response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn moderator_cannot_create_api_token() {
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
        .uri("/api/admin/api-tokens")
        .insert_header(("Authorization", token))
        .set_json(json!({ "tenant_slug": seed.tenant_slug, "name": "mod-token" }))
        .to_request();
    let create_response = test::call_service(&app, create_request).await;
    assert_eq!(create_response.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn api_token_with_scope_can_read_post_and_revoked_token_is_rejected() {
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

    let admin_token = bearer_for(
        &seed.admin_subject,
        "admin@example.com",
        "Admin",
        &settings.rooiam_jwt_secret,
    );

    let create_request = test::TestRequest::post()
        .uri("/api/admin/api-tokens")
        .insert_header(("Authorization", admin_token.clone()))
        .set_json(json!({
            "tenant_slug": seed.tenant_slug,
            "name": "Reader",
            "scopes": ["posts:read"]
        }))
        .to_request();
    let create_response = test::call_service(&app, create_request).await;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = read_json(create_response).await;
    let api_token = created
        .get("token")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    let token_id = created
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    let read_request = test::TestRequest::get()
        .uri(&format!(
            "/api/posts/{}?tenant_slug={}",
            seed.canonical_post_id, seed.tenant_slug
        ))
        .insert_header(("Authorization", format!("Bearer {api_token}")))
        .to_request();
    let read_response = test::call_service(&app, read_request).await;
    assert_eq!(read_response.status(), StatusCode::OK);

    let revoke_request = test::TestRequest::patch()
        .uri(&format!("/api/admin/api-tokens/{token_id}/revoke"))
        .insert_header(("Authorization", admin_token))
        .to_request();
    let revoke_response = test::call_service(&app, revoke_request).await;
    assert_eq!(revoke_response.status(), StatusCode::OK);

    let revoked_request = test::TestRequest::get()
        .uri(&format!(
            "/api/posts/{}?tenant_slug={}",
            seed.canonical_post_id, seed.tenant_slug
        ))
        .insert_header(("Authorization", format!("Bearer {api_token}")))
        .to_request();
    let revoked_response = test::call_service(&app, revoked_request).await;
    assert_eq!(revoked_response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn api_token_without_posts_read_scope_is_forbidden() {
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

    let admin_token = bearer_for(
        &seed.admin_subject,
        "admin@example.com",
        "Admin",
        &settings.rooiam_jwt_secret,
    );

    let create_request = test::TestRequest::post()
        .uri("/api/admin/api-tokens")
        .insert_header(("Authorization", admin_token))
        .set_json(json!({
            "tenant_slug": seed.tenant_slug,
            "name": "Roadmap only",
            "scopes": ["roadmap:read"]
        }))
        .to_request();
    let create_response = test::call_service(&app, create_request).await;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = read_json(create_response).await;
    let api_token = created.get("token").and_then(|v| v.as_str()).unwrap();

    let read_request = test::TestRequest::get()
        .uri(&format!(
            "/api/posts/{}?tenant_slug={}",
            seed.canonical_post_id, seed.tenant_slug
        ))
        .insert_header(("Authorization", format!("Bearer {api_token}")))
        .to_request();
    let read_response = test::call_service(&app, read_request).await;
    assert_eq!(read_response.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn api_token_cannot_access_other_tenant() {
    let settings = test_settings();
    let _guard = lock_test_db().await;
    let pool = db::establish_connection(&settings.database_url)
        .await
        .unwrap();
    reset_db(&pool).await;
    let seed = seed_basic_tenant(&pool).await;

    let tenant_b_id = uuid::Uuid::new_v4();
    let post_b_id = uuid::Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO tenants (id, slug, name) VALUES ($1, 'tenant-b-4', 'B')",
        tenant_b_id,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!(
        "INSERT INTO posts (id, tenant_id, board_id, user_id, title, body, status) SELECT $1, $2, b.id, $3, 'B Post', 'body', 'planned' FROM boards b WHERE b.tenant_id = $2 LIMIT 1",
        post_b_id,
        tenant_b_id,
        seed.admin_user_id
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

    let admin_token = bearer_for(
        &seed.admin_subject,
        "admin@example.com",
        "Admin",
        &settings.rooiam_jwt_secret,
    );

    let create_request = test::TestRequest::post()
        .uri("/api/admin/api-tokens")
        .insert_header(("Authorization", admin_token.clone()))
        .set_json(json!({
            "tenant_slug": seed.tenant_slug,
            "name": "Tenant A Token",
            "scopes": ["posts:read"]
        }))
        .to_request();
    let create_response = test::call_service(&app, create_request).await;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = read_json(create_response).await;
    let api_token = created.get("token").and_then(|v| v.as_str()).unwrap();

    let read_request = test::TestRequest::get()
        .uri(&format!("/api/posts/{post_b_id}?tenant_slug=tenant-b-4"))
        .insert_header(("Authorization", format!("Bearer {api_token}")))
        .to_request();
    let read_response = test::call_service(&app, read_request).await;
    assert!(read_response.status().is_client_error());
}

#[actix_web::test]
async fn expired_api_token_is_rejected() {
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

    let admin_token = bearer_for(
        &seed.admin_subject,
        "admin@example.com",
        "Admin",
        &settings.rooiam_jwt_secret,
    );

    let create_request = test::TestRequest::post()
        .uri("/api/admin/api-tokens")
        .insert_header(("Authorization", admin_token.clone()))
        .set_json(json!({
            "tenant_slug": seed.tenant_slug,
            "name": "Expiring Token",
            "scopes": ["posts:read"]
        }))
        .to_request();
    let create_response = test::call_service(&app, create_request).await;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = read_json(create_response).await;
    let api_token = created.get("token").and_then(|v| v.as_str()).unwrap();
    let token_id = created.get("id").and_then(|v| v.as_str()).unwrap();

    sqlx::query!(
        "UPDATE api_tokens SET expires_at = NOW() - INTERVAL '1 day' WHERE id = $1",
        uuid::Uuid::parse_str(token_id).unwrap()
    )
    .execute(&pool)
    .await
    .unwrap();

    let read_request = test::TestRequest::get()
        .uri(&format!(
            "/api/posts/{}?tenant_slug={}",
            seed.canonical_post_id, seed.tenant_slug
        ))
        .insert_header(("Authorization", format!("Bearer {api_token}")))
        .to_request();
    let read_response = test::call_service(&app, read_request).await;
    assert_eq!(read_response.status(), StatusCode::UNAUTHORIZED);
}
