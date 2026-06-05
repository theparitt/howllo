use actix_web::{http::StatusCode, test, web, App};

use crate::db;
use crate::http::test_support::{
    bearer_for, lock_test_db, reset_db, seed_basic_tenant, test_settings,
};
use crate::startup;

#[actix_web::test]
async fn admin_can_export_posts_as_json_and_csv() {
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

    let json_request = test::TestRequest::get()
        .uri(&format!(
            "/api/admin/export/posts.json?tenant_slug={}",
            seed.tenant_slug
        ))
        .insert_header(("Authorization", token.clone()))
        .to_request();
    let json_response = test::call_service(&app, json_request).await;
    assert_eq!(json_response.status(), StatusCode::OK);

    let csv_request = test::TestRequest::get()
        .uri(&format!(
            "/api/admin/export/posts.csv?tenant_slug={}",
            seed.tenant_slug
        ))
        .insert_header(("Authorization", token))
        .to_request();
    let csv_response = test::call_service(&app, csv_request).await;
    assert_eq!(csv_response.status(), StatusCode::OK);
    assert_eq!(
        csv_response.headers().get("content-type").unwrap(),
        "text/csv; charset=utf-8"
    );
}
