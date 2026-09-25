use std::{net::TcpListener, sync::Arc};

use actix_web::{http::StatusCode, test, web, App, HttpResponse, HttpServer};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::{
    db,
    http::test_support::{lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings},
    startup,
};

#[derive(Clone)]
struct Mock {
    issuer: String,
    token: Arc<RwLock<String>>,
    jwks: Arc<RwLock<String>>,
}

async fn discovery(mock: web::Data<Mock>) -> HttpResponse {
    let origin = mock.issuer.trim_end_matches('/');
    HttpResponse::Ok().json(json!({
        "issuer": mock.issuer,
        "authorization_endpoint": format!("{origin}/authorize"),
        "token_endpoint": format!("{origin}/token"),
        "jwks_uri": format!("{origin}/jwks"),
    }))
}
async fn jwks(mock: web::Data<Mock>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(mock.jwks.read().await.clone())
}
async fn token(
    mock: web::Data<Mock>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if form.get("client_id").map(String::as_str) == Some("backup-client") {
        assert_eq!(
            form.get("client_secret").map(String::as_str),
            Some("backup-secret")
        );
    }
    HttpResponse::Ok().json(json!({"id_token": mock.token.read().await.clone()}))
}

fn signed(claims: Value) -> String {
    let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("test-key".into());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(include_bytes!("test_oidc_key.pem")).unwrap(),
    )
    .unwrap()
}

fn signed_rotated(claims: Value) -> String {
    let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("rotated-key".into());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(include_bytes!("test_oidc_rotated_key.pem")).unwrap(),
    )
    .unwrap()
}

#[actix_web::test]
async fn oidc_login_validates_state_token_and_issues_a_howllo_session() {
    let _guard = lock_test_db().await;
    let settings = test_settings();
    let pool = db::establish_connection(&settings.database_url)
        .await
        .unwrap();
    reset_db(&pool).await;
    let seed = seed_basic_tenant(&pool).await;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    // Some providers publish an issuer ending in '/', which must match exactly.
    let issuer = format!(
        "http://127.0.0.1:{}/",
        listener.local_addr().unwrap().port()
    );
    let mock = Mock {
        issuer: issuer.clone(),
        token: Arc::new(RwLock::new(String::new())),
        jwks: Arc::new(RwLock::new(include_str!("test_oidc_jwk.json").into())),
    };
    let mock_data = web::Data::new(mock.clone());
    let server = HttpServer::new(move || {
        App::new()
            .app_data(mock_data.clone())
            .route(
                "/.well-known/openid-configuration",
                web::get().to(discovery),
            )
            .route("/jwks", web::get().to(jwks))
            .route("/token", web::post().to(token))
    })
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    actix_web::rt::spawn(server);

    std::env::set_var("HOWLLO_OIDC_PROVIDERS", json!([
        {"id":"company","display_name":"Company SSO","issuer":issuer,"client_id":"howllo-test","client_secret":"test-secret","enabled":true},
        {"id":"backup","display_name":"Backup SSO","issuer":issuer,"client_id":"backup-client","client_secret":"backup-secret","token_endpoint_auth_method":"client_secret_post","enabled":true},
        {"id":"disabled","display_name":"Disabled","issuer":issuer,"client_id":"disabled","enabled":false},
        {"id":"unavailable","display_name":"Unavailable","issuer":"http://127.0.0.1:9","client_id":"unavailable","enabled":true},
        {"id":"invalid-discovery","display_name":"Invalid","issuer":format!("{issuer}/invalid"),"client_id":"invalid","enabled":true}
    ]).to_string());
    std::env::set_var("HOWLLO_PUBLIC_API_URL", "http://127.0.0.1:7700");
    std::env::set_var("HOWLLO_WEB_ORIGIN", "http://localhost:7702");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(settings))
            .configure(startup::configure),
    )
    .await;

    macro_rules! start_login {
        () => {{
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri("/api/auth/login/company")
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::FOUND);
            let location = url::Url::parse(
                response
                    .headers()
                    .get("Location")
                    .unwrap()
                    .to_str()
                    .unwrap(),
            )
            .unwrap();
            let state = location
                .query_pairs()
                .find(|(key, _)| key == "state")
                .unwrap()
                .1
                .to_string();
            let nonce = location
                .query_pairs()
                .find(|(key, _)| key == "nonce")
                .unwrap()
                .1
                .to_string();
            let cookie = response
                .headers()
                .get("Set-Cookie")
                .unwrap()
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_string();
            (state, nonce, cookie)
        }};
    }

    let providers = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/auth/providers")
            .to_request(),
    )
    .await;
    let listing = read_json(providers).await;
    assert!(listing
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == "company"));
    assert!(listing
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == "backup"));
    assert!(!listing
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == "disabled"));
    assert!(!listing.to_string().contains("test-secret"));
    for id in ["unknown", "disabled"] {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/auth/login/{id}"))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let backup = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/auth/login/backup")
            .to_request(),
    )
    .await;
    assert_eq!(backup.status(), StatusCode::FOUND);
    let unavailable = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/auth/login/unavailable")
            .to_request(),
    )
    .await;
    assert_eq!(unavailable.status(), StatusCode::UNAUTHORIZED);
    let invalid_discovery = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/auth/login/invalid-discovery")
            .to_request(),
    )
    .await;
    assert_eq!(invalid_discovery.status(), StatusCode::UNAUTHORIZED);

    // Every rejected token gets a fresh login transaction because state is one-time.
    for invalid in [
        "nonce",
        "issuer",
        "audience",
        "expired",
        "signature",
        "missing_code",
    ] {
        let (state, nonce, cookie) = start_login!();
        let now = chrono::Utc::now().timestamp();
        let claims = json!({"sub":"subject-1", "iss": if invalid == "issuer" {"http://wrong.invalid"} else {issuer.as_str()},
            "aud": if invalid == "audience" {"wrong-client"} else {"howllo-test"},
            "exp": if invalid == "expired" {now-3600} else {now+3600}, "iat":now,
            "nonce": if invalid == "nonce" {"wrong-nonce"} else {nonce.as_str()}, "email":"person@example.com", "email_verified":true, "name":"Person"});
        let mut id_token = signed(claims);
        if invalid == "signature" {
            id_token.push('x');
        }
        *mock.token.write().await = id_token;
        let code = if invalid == "missing_code" {
            ""
        } else {
            "test-code"
        };
        let uri = format!("/api/auth/callback/company?state={state}&code={code}");
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&uri)
                .insert_header(("Cookie", cookie))
                .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "case {invalid}"
        );
    }

    let (state, _, cookie) = start_login!();
    let missing_cookie = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/auth/callback/company?state={state}&code=x"))
            .to_request(),
    )
    .await;
    assert_eq!(missing_cookie.status(), StatusCode::UNAUTHORIZED);
    let missing_state = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/auth/callback/company?code=x")
            .insert_header(("Cookie", cookie.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(missing_state.status(), StatusCode::UNAUTHORIZED);
    let wrong_state = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/auth/callback/company?state=wrong&code=x")
            .insert_header(("Cookie", cookie.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(wrong_state.status(), StatusCode::UNAUTHORIZED);
    let provider_error = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/auth/callback/company?state={state}&error=access_denied"
            ))
            .insert_header(("Cookie", cookie))
            .to_request(),
    )
    .await;
    assert_eq!(provider_error.status(), StatusCode::UNAUTHORIZED);

    let (state, nonce, cookie) = start_login!();
    let now = chrono::Utc::now().timestamp();
    *mock.token.write().await = signed(
        json!({"sub":"subject-1","iss":issuer,"aud":"howllo-test","exp":now+3600,"iat":now,"nonce":nonce,"email":"person@example.com","email_verified":true,"name":"Person"}),
    );
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/auth/callback/company?state={state}&code=test-code"
            ))
            .insert_header(("Cookie", cookie))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FOUND);
    let redirect = response
        .headers()
        .get("Location")
        .unwrap()
        .to_str()
        .unwrap();
    let code = url::Url::parse(redirect)
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "exchange_code")
        .unwrap()
        .1
        .to_string();
    let exchange = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/exchange")
            .set_json(json!({"code":code}))
            .to_request(),
    )
    .await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let payload = read_json(exchange).await;
    assert_eq!(payload["return_to"], "/");
    let account_token = payload["access_token"].as_str().unwrap();
    let replay = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/exchange")
            .set_json(json!({"code":code}))
            .to_request(),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    let workspace = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/workspace-session")
            .insert_header(("Authorization", format!("Bearer {account_token}")))
            .set_json(json!({"tenant_slug":seed.tenant_slug}))
            .to_request(),
    )
    .await;
    assert_eq!(workspace.status(), StatusCode::CREATED);
    let id_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_identities WHERE provider_id = 'company' AND subject = 'subject-1'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(id_count, 1);

    // The second configured provider uses the same adapter and gets a distinct identity.
    let backup_start = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/auth/login/backup")
            .to_request(),
    )
    .await;
    let backup_location = url::Url::parse(
        backup_start
            .headers()
            .get("Location")
            .unwrap()
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let backup_state = backup_location
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .to_string();
    let backup_nonce = backup_location
        .query_pairs()
        .find(|(key, _)| key == "nonce")
        .unwrap()
        .1
        .to_string();
    let backup_cookie = backup_start
        .headers()
        .get("Set-Cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    *mock.jwks.write().await = include_str!("test_oidc_rotated_jwk.json").into();
    *mock.token.write().await = signed_rotated(
        json!({"sub":"subject-1","iss":issuer,"aud":"backup-client","exp":now+3600,"iat":now,"nonce":backup_nonce}),
    );
    let backup_callback = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/auth/callback/backup?state={backup_state}&code=test-code"
            ))
            .insert_header(("Cookie", backup_cookie))
            .to_request(),
    )
    .await;
    assert_eq!(backup_callback.status(), StatusCode::FOUND);
    let distinct: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT user_id) FROM user_identities WHERE subject = 'subject-1' AND provider_id IN ('company', 'backup')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(distinct, 2);

    std::env::remove_var("HOWLLO_OIDC_PROVIDERS");
    std::env::remove_var("HOWLLO_PUBLIC_API_URL");
    std::env::remove_var("HOWLLO_WEB_ORIGIN");
    handle.stop(true).await;
}
