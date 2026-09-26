//! Approved public-board appearance plugins. No tenant-supplied code runs here.
use actix_web::{get, patch, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::{local_admin, require_permission, AuthenticatedUser},
    db::DbPool,
    domain::permission::Permission,
    errors::AppError,
};

#[derive(Debug, Serialize, FromRow)]
struct PluginRow {
    id: String,
    version: String,
    name: String,
    description: String,
    slot: String,
    stylesheet_path: String,
    is_approved: bool,
}

#[derive(Debug, Serialize, FromRow)]
struct WorkspacePluginRow {
    id: String,
    version: String,
    name: String,
    description: String,
    slot: String,
    stylesheet_path: String,
    enabled: bool,
}

#[derive(Deserialize)]
pub struct SetEnabled {
    enabled: bool,
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(%error, "plugin storage error");
    AppError::InternalServerError
}

async fn tenant_id(pool: &DbPool, slug: &str) -> Result<Uuid, AppError> {
    crate::repositories::membership_repository::resolve_tenant_id(pool, slug).await
}

async fn manage_tenant(pool: &DbPool, slug: &str, user_id: Uuid) -> Result<Uuid, AppError> {
    let id = tenant_id(pool, slug).await?;
    require_permission(pool, id, user_id, Permission::ManageSettings).await?;
    Ok(id)
}

#[get("/api/admin/plugins")]
pub async fn list_platform_plugins(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    if !local_admin::is_local_admin_user(&auth.0) {
        return Err(AppError::Forbidden);
    }
    let plugins = sqlx::query_as::<_, PluginRow>(
        "SELECT id, version, name, description, slot, stylesheet_path, is_approved FROM plugin_catalog ORDER BY slot, name"
    ).fetch_all(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(plugins))
}

#[patch("/api/admin/plugins/{id}")]
pub async fn set_platform_plugin_approval(
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    body: web::Json<SetEnabled>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    if !local_admin::is_local_admin_user(&auth.0) {
        return Err(AppError::Forbidden);
    }
    let mut tx = pool.begin().await.map_err(db_error)?;
    let updated = sqlx::query("UPDATE plugin_catalog SET is_approved=$1 WHERE id=$2")
        .bind(body.enabled)
        .bind(path.as_str())
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    if !body.enabled {
        sqlx::query("UPDATE workspace_plugins SET enabled=FALSE, updated_at=NOW() WHERE plugin_id=$1 AND enabled=TRUE")
            .bind(path.as_str()).execute(&mut *tx).await.map_err(db_error)?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(HttpResponse::NoContent().finish())
}

#[get("/api/tenants/{slug}/plugins")]
pub async fn list_workspace_plugins(
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant = manage_tenant(pool.get_ref(), &path, auth.0.id).await?;
    let plugins = sqlx::query_as::<_, WorkspacePluginRow>(
        "SELECT p.id, p.version, p.name, p.description, p.slot, p.stylesheet_path, COALESCE(w.enabled,FALSE) AS enabled FROM plugin_catalog p LEFT JOIN workspace_plugins w ON w.plugin_id=p.id AND w.tenant_id=$1 WHERE p.is_approved=TRUE ORDER BY p.slot,p.name"
    ).bind(tenant).fetch_all(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(plugins))
}

#[put("/api/tenants/{slug}/plugins/{id}")]
pub async fn set_workspace_plugin(
    pool: web::Data<DbPool>,
    path: web::Path<(String, String)>,
    body: web::Json<SetEnabled>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let (slug, plugin_id) = path.into_inner();
    let tenant = manage_tenant(pool.get_ref(), &slug, auth.0.id).await?;
    let mut tx = pool.begin().await.map_err(db_error)?;
    // Hold the catalog row while enabling so a concurrent platform revocation
    // cannot leave an installation active after approval is removed.
    let row: Option<(String, bool)> =
        sqlx::query_as("SELECT slot,is_approved FROM plugin_catalog WHERE id=$1 FOR SHARE")
            .bind(&plugin_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
    let (slot, approved) = row.ok_or(AppError::NotFound)?;
    if body.enabled && !approved {
        return Err(AppError::Forbidden);
    }
    if body.enabled {
        // The partial unique index also protects this invariant under concurrent requests.
        sqlx::query("UPDATE workspace_plugins SET enabled=FALSE, updated_at=NOW() WHERE tenant_id=$1 AND slot=$2 AND enabled=TRUE")
            .bind(tenant).bind(&slot).execute(&mut *tx).await.map_err(db_error)?;
    }
    sqlx::query("INSERT INTO workspace_plugins (tenant_id,plugin_id,slot,enabled) VALUES ($1,$2,$3,$4) ON CONFLICT (tenant_id,plugin_id) DO UPDATE SET enabled=$4, updated_at=NOW()")
        .bind(tenant).bind(&plugin_id).bind(&slot).bind(body.enabled)
        .execute(&mut *tx).await.map_err(|error| {
            if let sqlx::Error::Database(db) = &error { if db.is_unique_violation() { return AppError::Validation("Another plugin is already enabled for this slot".into()); } }
            db_error(error)
        })?;
    tx.commit().await.map_err(db_error)?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Serialize, FromRow)]
struct PublicBoard {
    slug: String,
    name: String,
    description: Option<String>,
    board_type: String,
}

#[derive(Serialize, FromRow)]
struct ActivePlugin {
    id: String,
    version: String,
    slot: String,
    stylesheet_path: String,
}

#[get("/api/plugins/v1/workspaces/{slug}/context")]
pub async fn public_context(
    pool: web::Data<DbPool>,
    path: web::Path<String>,
) -> Result<impl Responder, AppError> {
    let slug = path.into_inner();
    let workspace: Option<(Uuid, String, String, Option<String>, Option<String>, bool, bool, bool)> = sqlx::query_as(
        "SELECT t.id,t.name,COALESCE(b.site_name,t.name),b.accent_color,b.background_color,COALESCE(b.show_boards,TRUE),COALESCE(b.show_feed,TRUE),COALESCE(b.show_roadmap,TRUE) FROM tenants t LEFT JOIN tenant_branding b ON b.tenant_id=t.id WHERE t.slug=$1 AND t.is_published=TRUE"
    ).bind(&slug).fetch_optional(pool.get_ref()).await.map_err(db_error)?;
    let (id, name, site_name, accent_color, background_color, show_boards, show_feed, show_roadmap) =
        workspace.ok_or(AppError::NotFound)?;
    let boards: Vec<PublicBoard> = if show_boards {
        sqlx::query_as("SELECT slug,name,description,board_type FROM boards WHERE tenant_id=$1 AND is_enabled=TRUE AND is_private=FALSE ORDER BY created_at")
            .bind(id).fetch_all(pool.get_ref()).await.map_err(db_error)?
    } else {
        vec![]
    };
    let plugins: Vec<ActivePlugin> = sqlx::query_as(
        "SELECT p.id,p.version,p.slot,p.stylesheet_path FROM workspace_plugins w JOIN plugin_catalog p ON p.id=w.plugin_id WHERE w.tenant_id=$1 AND w.enabled=TRUE AND p.is_approved=TRUE ORDER BY p.slot"
    ).bind(id).fetch_all(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(serde_json::json!({
        "api_version": 1,
        "workspace": { "slug": slug, "name": name, "site_name": site_name, "accent_color": accent_color, "background_color": background_color,
            "pages": { "boards": show_boards, "feed": show_feed, "roadmap": show_roadmap } },
        "boards": boards,
        "plugins": plugins,
    })))
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
    async fn only_approved_workspace_plugins_and_public_boards_reach_context() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let own = seed_basic_tenant(&pool).await;
        let other = seed_basic_tenant(&pool).await;
        let admin = bearer_for(
            &own.admin_subject,
            &format!("{}@example.com", own.admin_subject),
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
        let cannot_approve = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri("/api/admin/plugins/editorial-type")
                .insert_header(("Authorization", admin.clone()))
                .set_json(json!({"enabled":false}))
                .to_request(),
        )
        .await;
        assert_eq!(cannot_approve.status(), StatusCode::FORBIDDEN);
        let context_url = format!("/api/plugins/v1/workspaces/{}/context", own.tenant_slug);
        let initial = read_json(
            test::call_service(
                &app,
                test::TestRequest::get().uri(&context_url).to_request(),
            )
            .await,
        )
        .await;
        assert_eq!(initial["api_version"], 1);
        assert_eq!(initial["boards"].as_array().unwrap().len(), 1);
        assert_eq!(initial["boards"][0]["slug"], own.board_slug);
        assert!(!initial.to_string().contains(&own.private_board_slug));
        assert!(!initial.to_string().contains(&own.member_subject));

        let url = format!("/api/tenants/{}/plugins/editorial-type", own.tenant_slug);
        let enable = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&url)
                .insert_header(("Authorization", admin.clone()))
                .set_json(json!({"enabled":true}))
                .to_request(),
        )
        .await;
        assert_eq!(enable.status(), StatusCode::NO_CONTENT);
        let second = read_json(
            test::call_service(
                &app,
                test::TestRequest::get().uri(&context_url).to_request(),
            )
            .await,
        )
        .await;
        assert_eq!(second["plugins"][0]["id"], "editorial-type");

        let replace = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!(
                    "/api/tenants/{}/plugins/clean-type",
                    own.tenant_slug
                ))
                .insert_header(("Authorization", admin.clone()))
                .set_json(json!({"enabled":true}))
                .to_request(),
        )
        .await;
        assert_eq!(replace.status(), StatusCode::NO_CONTENT);
        let third = read_json(
            test::call_service(
                &app,
                test::TestRequest::get().uri(&context_url).to_request(),
            )
            .await,
        )
        .await;
        assert_eq!(third["plugins"].as_array().unwrap().len(), 1);
        assert_eq!(third["plugins"][0]["id"], "clean-type");

        let cross_tenant = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!(
                    "/api/tenants/{}/plugins/board-grid",
                    other.tenant_slug
                ))
                .insert_header(("Authorization", admin))
                .set_json(json!({"enabled":true}))
                .to_request(),
        )
        .await;
        assert_eq!(cross_tenant.status(), StatusCode::FORBIDDEN);

        sqlx::query("UPDATE plugin_catalog SET is_approved=FALSE WHERE id='clean-type'")
            .execute(&pool)
            .await
            .unwrap();
        let revoked = read_json(
            test::call_service(
                &app,
                test::TestRequest::get().uri(&context_url).to_request(),
            )
            .await,
        )
        .await;
        assert!(revoked["plugins"].as_array().unwrap().is_empty());
        sqlx::query("UPDATE tenants SET is_published=FALSE WHERE slug=$1")
            .bind(&own.tenant_slug)
            .execute(&pool)
            .await
            .unwrap();
        let unpublished = test::call_service(
            &app,
            test::TestRequest::get().uri(&context_url).to_request(),
        )
        .await;
        assert_eq!(unpublished.status(), StatusCode::NOT_FOUND);
        sqlx::query("UPDATE plugin_catalog SET is_approved=TRUE WHERE id='clean-type'")
            .execute(&pool)
            .await
            .unwrap();
    }
}
