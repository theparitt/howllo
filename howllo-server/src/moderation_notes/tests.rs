use actix_web::{http::StatusCode, test, web, App};
use serde_json::json;

use crate::db;
use crate::http::test_support::{
    bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
};
use crate::startup;

#[actix_web::test]
async fn admin_can_create_and_list_post_notes() {
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
        .uri(&format!(
            "/api/admin/posts/{}/notes",
            seed.canonical_post_id
        ))
        .insert_header(("Authorization", token.clone()))
        .set_json(json!({ "body": "Needs product review" }))
        .to_request();
    let create_response = test::call_service(&app, create_request).await;
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let list_request = test::TestRequest::get()
        .uri(&format!(
            "/api/admin/posts/{}/notes",
            seed.canonical_post_id
        ))
        .insert_header(("Authorization", token))
        .to_request();
    let list_response = test::call_service(&app, list_request).await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = read_json(list_response).await;
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].get("body").and_then(|v| v.as_str()),
        Some("Needs product review")
    );
}

#[actix_web::test]
async fn moderator_can_create_and_list_post_notes() {
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
        .uri(&format!(
            "/api/admin/posts/{}/notes",
            seed.canonical_post_id
        ))
        .insert_header(("Authorization", token.clone()))
        .set_json(json!({ "body": "Moderator note" }))
        .to_request();
    let create_response = test::call_service(&app, create_request).await;
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let list_request = test::TestRequest::get()
        .uri(&format!(
            "/api/admin/posts/{}/notes",
            seed.canonical_post_id
        ))
        .insert_header(("Authorization", token))
        .to_request();
    let list_response = test::call_service(&app, list_request).await;
    assert_eq!(list_response.status(), StatusCode::OK);
}
