//! Small, safe board presentation settings and staff topic controls.
use actix_web::{get, patch, put, web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    audit::{record_in_tx, AuditEntry},
    auth::{require_permission, AuthenticatedUser},
    db::DbPool,
    domain::permission::Permission,
    errors::AppError,
    plugins::registry,
    services::board_service,
};

#[derive(Serialize, FromRow)]
struct Presentation {
    announcement: String,
    sidebar_text: String,
    footer_text: String,
}

#[derive(Deserialize)]
pub struct PresentationInput {
    announcement: String,
    sidebar_text: String,
    footer_text: String,
}

#[derive(Deserialize)]
pub struct TenantQuery {
    tenant_slug: String,
}

#[derive(Serialize, FromRow)]
struct BoardPlugin {
    id: String,
    version: String,
    name: String,
    description: String,
    slot: String,
    stylesheet_path: String,
    enabled: bool,
}

#[derive(Deserialize)]
pub struct ToggleInput {
    enabled: bool,
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(%error, "forum board storage error");
    AppError::InternalServerError
}

async fn board_tenant(
    pool: &DbPool,
    board_id: Uuid,
    user_id: Uuid,
    permission: Permission,
) -> Result<Uuid, AppError> {
    let tenant: Option<Uuid> = sqlx::query_scalar("SELECT tenant_id FROM boards WHERE id=$1")
        .bind(board_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    let tenant = tenant.ok_or(AppError::NotFound)?;
    require_permission(pool, tenant, user_id, permission).await?;
    Ok(tenant)
}

#[get("/api/boards/{slug}/presentation")]
pub async fn get_public_presentation(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    query: web::Query<TenantQuery>,
) -> Result<impl Responder, AppError> {
    let board =
        board_service::get_board_detail(&req, pool.get_ref(), &query.tenant_slug, &path).await?;
    let presentation: Option<Presentation> = sqlx::query_as(
        "SELECT announcement,sidebar_text,footer_text FROM board_presentation WHERE board_id=$1",
    )
    .bind(board.id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(db_error)?;
    let plugins: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT p.id,p.version,p.slot,p.stylesheet_path FROM board_plugins bp JOIN plugin_catalog p ON p.id=bp.plugin_id WHERE bp.board_id=$1 AND bp.enabled=TRUE AND p.is_approved=TRUE ORDER BY p.slot"
    ).bind(board.id).fetch_all(pool.get_ref()).await.map_err(db_error)?;
    let presentation = presentation.unwrap_or(Presentation {
        announcement: String::new(),
        sidebar_text: String::new(),
        footer_text: String::new(),
    });
    Ok(HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(serde_json::json!({
        "announcement": presentation.announcement,
        "sidebar_text": presentation.sidebar_text,
        "footer_text": presentation.footer_text,
        "plugins": plugins.into_iter().filter(|(id, version, slot, path)| registry::matches(id, version, slot, path)).map(|(id, _, _, stylesheet_path)| serde_json::json!({ "id": id, "stylesheet_path": stylesheet_path })).collect::<Vec<_>>()
    })))
}

#[get("/api/admin/boards/{id}/presentation")]
pub async fn get_admin_presentation(
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let id = path.into_inner();
    board_tenant(pool.get_ref(), id, auth.0.id, Permission::ManageBoards).await?;
    let presentation: Option<Presentation> = sqlx::query_as(
        "SELECT announcement,sidebar_text,footer_text FROM board_presentation WHERE board_id=$1",
    )
    .bind(id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(db_error)?;
    Ok(
        HttpResponse::Ok().json(presentation.unwrap_or(Presentation {
            announcement: String::new(),
            sidebar_text: String::new(),
            footer_text: String::new(),
        })),
    )
}

#[put("/api/admin/boards/{id}/presentation")]
pub async fn put_admin_presentation(
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<PresentationInput>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let id = path.into_inner();
    board_tenant(pool.get_ref(), id, auth.0.id, Permission::ManageBoards).await?;
    let fields = [&body.announcement, &body.sidebar_text, &body.footer_text];
    if fields.iter().any(|value| value.chars().count() > 1000) {
        return Err(AppError::Validation(
            "Board text is limited to 1,000 characters per field".into(),
        ));
    }
    sqlx::query("INSERT INTO board_presentation (board_id,announcement,sidebar_text,footer_text) VALUES ($1,$2,$3,$4) ON CONFLICT (board_id) DO UPDATE SET announcement=$2,sidebar_text=$3,footer_text=$4,updated_at=NOW()")
        .bind(id).bind(body.announcement.trim()).bind(body.sidebar_text.trim()).bind(body.footer_text.trim())
        .execute(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::NoContent().finish())
}

#[get("/api/admin/boards/{id}/plugins")]
pub async fn list_board_plugins(
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let id = path.into_inner();
    board_tenant(pool.get_ref(), id, auth.0.id, Permission::ManageBoards).await?;
    let mut plugins: Vec<BoardPlugin> = sqlx::query_as("SELECT p.id,p.version,p.name,p.description,p.slot,p.stylesheet_path,COALESCE(bp.enabled,FALSE) AS enabled FROM plugin_catalog p LEFT JOIN board_plugins bp ON bp.plugin_id=p.id AND bp.board_id=$1 WHERE p.is_approved=TRUE AND p.slot LIKE 'board.topics.%' ORDER BY p.slot,p.name")
        .bind(id).fetch_all(pool.get_ref()).await.map_err(db_error)?;
    plugins.retain(|p| registry::matches(&p.id, &p.version, &p.slot, &p.stylesheet_path));
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(plugins))
}

#[put("/api/admin/boards/{id}/plugins/{plugin_id}")]
pub async fn toggle_board_plugin(
    pool: web::Data<DbPool>,
    path: web::Path<(Uuid, String)>,
    body: web::Json<ToggleInput>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let (board_id, plugin_id) = path.into_inner();
    let built_in = registry::find(&plugin_id).ok_or(AppError::NotFound)?;
    board_tenant(
        pool.get_ref(),
        board_id,
        auth.0.id,
        Permission::ManageBoards,
    )
    .await?;
    let mut tx = pool.begin().await.map_err(db_error)?;
    let row: Option<(String, bool)> =
        sqlx::query_as("SELECT slot,is_approved FROM plugin_catalog WHERE id=$1 AND version=$2 AND slot=$3 AND stylesheet_path=$4 FOR SHARE")
            .bind(&plugin_id)
            .bind(built_in.version).bind(built_in.slot).bind(built_in.stylesheet_path)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
    let (slot, approved) = row.ok_or(AppError::NotFound)?;
    if !slot.starts_with("board.topics.") {
        return Err(AppError::Validation(
            "This plugin cannot be enabled on a board".into(),
        ));
    }
    if body.enabled && !approved {
        return Err(AppError::Forbidden);
    }
    if body.enabled {
        sqlx::query("UPDATE board_plugins SET enabled=FALSE,updated_at=NOW() WHERE board_id=$1 AND slot=$2 AND enabled=TRUE")
            .bind(board_id).bind(&slot).execute(&mut *tx).await.map_err(db_error)?;
    }
    sqlx::query("INSERT INTO board_plugins (board_id,plugin_id,slot,enabled) VALUES ($1,$2,$3,$4) ON CONFLICT (board_id,plugin_id) DO UPDATE SET enabled=$4,updated_at=NOW()")
        .bind(board_id).bind(&plugin_id).bind(&slot).bind(body.enabled).execute(&mut *tx).await.map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(HttpResponse::NoContent().finish())
}

#[patch("/api/admin/posts/{id}/pin")]
pub async fn pin_post(
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<ToggleInput>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let id = path.into_inner();
    let post: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT tenant_id,(pinned_at IS NOT NULL) FROM posts WHERE id=$1 AND deleted_at IS NULL AND is_hidden=FALSE",
    )
    .bind(id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(db_error)?;
    let (tenant, was_pinned) = post.ok_or(AppError::NotFound)?;
    require_permission(
        pool.get_ref(),
        tenant,
        auth.0.id,
        Permission::ModerateContent,
    )
    .await?;
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("UPDATE posts SET pinned_at=CASE WHEN $2 THEN COALESCE(pinned_at,NOW()) ELSE NULL END,updated_at=NOW() WHERE id=$1")
        .bind(id).bind(body.enabled).execute(&mut *tx).await.map_err(db_error)?;
    if was_pinned != body.enabled {
        record_in_tx(
            &mut tx,
            AuditEntry {
                tenant_id: tenant,
                actor_user_id: auth.0.id,
                entity_type: "post",
                entity_id: id,
                action: if body.enabled { "pin" } else { "unpin" },
                old_value: Some(serde_json::json!({ "is_pinned": was_pinned })),
                new_value: Some(serde_json::json!({ "is_pinned": body.enabled })),
                reason: None,
            },
        )
        .await?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use crate::{
        db,
        http::test_support::{
            bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
        },
        startup,
    };
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;

    #[actix_web::test]
    async fn presentation_plugins_and_pins_stay_inside_the_board_and_workspace() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let own = seed_basic_tenant(&pool).await;
        let other = seed_basic_tenant(&pool).await;
        let board_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM boards WHERE slug=$1")
            .bind(&own.board_slug)
            .fetch_one(&pool)
            .await
            .unwrap();
        let own_admin = bearer_for(
            &own.admin_subject,
            "own@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let other_admin = bearer_for(
            &other.admin_subject,
            "other@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .app_data(web::Data::new(settings))
                .configure(startup::configure),
        )
        .await;

        let settings_path = format!("/api/admin/boards/{board_id}/presentation");
        let denied = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&settings_path)
                .insert_header(("Authorization", other_admin))
                .set_json(json!({"announcement":"Other","sidebar_text":"","footer_text":""}))
                .to_request(),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);

        let saved = test::call_service(&app, test::TestRequest::put().uri(&settings_path)
            .insert_header(("Authorization", own_admin.clone()))
            .set_json(json!({"announcement":"Board news","sidebar_text":"Community rules","footer_text":"Thanks for visiting"})).to_request()).await;
        assert_eq!(saved.status(), StatusCode::NO_CONTENT);

        let plugin_path = format!("/api/admin/boards/{board_id}/plugins/compact-topics");
        let enabled = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&plugin_path)
                .insert_header(("Authorization", own_admin.clone()))
                .set_json(json!({"enabled":true}))
                .to_request(),
        )
        .await;
        assert_eq!(enabled.status(), StatusCode::NO_CONTENT);

        let public_path = format!(
            "/api/boards/{}/presentation?tenant_slug={}",
            own.board_slug, own.tenant_slug
        );
        let visible = test::call_service(
            &app,
            test::TestRequest::get().uri(&public_path).to_request(),
        )
        .await;
        assert_eq!(visible.status(), StatusCode::OK);
        let body = read_json(visible).await;
        assert_eq!(body["announcement"], "Board news");
        assert_eq!(body["plugins"][0]["id"], "compact-topics");

        // An approved catalog row alone cannot introduce an external asset.
        sqlx::query("INSERT INTO plugin_catalog (id,version,name,description,slot,stylesheet_path,is_approved) VALUES ('unshipped-board','1.0.0','Unshipped','Not in Howllo','board.topics.typography','/plugins/unshipped-board.css',TRUE) ON CONFLICT (id) DO UPDATE SET is_approved=TRUE")
            .execute(&pool).await.unwrap();
        let rogue_toggle = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!(
                    "/api/admin/boards/{board_id}/plugins/unshipped-board"
                ))
                .insert_header(("Authorization", own_admin.clone()))
                .set_json(json!({"enabled":true}))
                .to_request(),
        )
        .await;
        assert_eq!(rogue_toggle.status(), StatusCode::NOT_FOUND);
        sqlx::query("INSERT INTO board_plugins (board_id,plugin_id,slot,enabled) VALUES ($1,'unshipped-board','board.topics.typography',TRUE)")
            .bind(board_id).execute(&pool).await.unwrap();
        let public_again = read_json(
            test::call_service(
                &app,
                test::TestRequest::get().uri(&public_path).to_request(),
            )
            .await,
        )
        .await;
        assert_eq!(public_again["plugins"].as_array().unwrap().len(), 1);
        sqlx::query("DELETE FROM plugin_catalog WHERE id='unshipped-board'")
            .execute(&pool)
            .await
            .unwrap();

        let private_path = format!(
            "/api/boards/{}/presentation?tenant_slug={}",
            own.private_board_slug, own.tenant_slug
        );
        let private = test::call_service(
            &app,
            test::TestRequest::get().uri(&private_path).to_request(),
        )
        .await;
        assert_eq!(private.status(), StatusCode::FORBIDDEN);

        let pin_path = format!("/api/admin/posts/{}/pin", own.canonical_post_id);
        let pinned = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&pin_path)
                .insert_header(("Authorization", own_admin))
                .set_json(json!({"enabled":true}))
                .to_request(),
        )
        .await;
        assert_eq!(pinned.status(), StatusCode::NO_CONTENT);
        let pinned_at: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT pinned_at FROM posts WHERE id=$1")
                .bind(own.canonical_post_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(pinned_at.is_some());
        let audit_action: String = sqlx::query_scalar("SELECT action FROM audit_logs WHERE entity_type='post' AND entity_id=$1 ORDER BY created_at DESC LIMIT 1")
            .bind(own.canonical_post_id).fetch_one(&pool).await.unwrap();
        assert_eq!(audit_action, "pin");
    }
}
