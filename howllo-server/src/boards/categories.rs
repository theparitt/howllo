use actix_web::{delete, get, patch, post, web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::{require_permission, AuthenticatedUser},
    db::DbPool,
    domain::permission::Permission,
    errors::AppError,
    services::board_service,
};

#[derive(Serialize, FromRow)]
pub struct BoardCategory {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub color: String,
}

#[derive(Deserialize)]
pub struct CategoryInput {
    pub slug: String,
    pub name: String,
    pub color: String,
}

#[derive(Deserialize)]
pub struct CategoryUpdate {
    pub name: String,
    pub color: String,
}

#[derive(Deserialize)]
pub struct PublicQuery {
    pub tenant_slug: String,
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(%error, "board category storage error");
    AppError::InternalServerError
}

fn valid_name_color(name: &str, color: &str) -> Result<(), AppError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err(AppError::Validation(
            "Category name must be 1–80 characters".into(),
        ));
    }
    if color.len() != 7
        || !color.starts_with('#')
        || !color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(AppError::Validation(
            "Category color must be a hex color".into(),
        ));
    }
    Ok(())
}

async fn manage_board(pool: &DbPool, board_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let tenant_id = crate::repositories::board_repository::get_board_tenant(pool, board_id)
        .await
        .map_err(db_error)?
        .ok_or(AppError::NotFound)?;
    require_permission(pool, tenant_id, user_id, Permission::ManageBoards).await?;
    Ok(())
}

async fn list(pool: &DbPool, board_id: Uuid) -> Result<Vec<BoardCategory>, AppError> {
    sqlx::query_as(
        "SELECT id,slug,name,color FROM board_categories WHERE board_id=$1 ORDER BY name",
    )
    .bind(board_id)
    .fetch_all(pool)
    .await
    .map_err(db_error)
}

#[get("/api/boards/{board_slug}/categories")]
pub async fn list_public(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    path: web::Path<String>,
    query: web::Query<PublicQuery>,
) -> Result<impl Responder, AppError> {
    let board =
        board_service::get_board_detail(&req, pool.get_ref(), &query.tenant_slug, &path).await?;
    Ok(HttpResponse::Ok().json(list(pool.get_ref(), board.id).await?))
}

#[get("/api/admin/boards/{board_id}/categories")]
pub async fn list_admin(
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    manage_board(pool.get_ref(), *path, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(list(pool.get_ref(), *path).await?))
}

#[post("/api/admin/boards/{board_id}/categories")]
pub async fn create(
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<CategoryInput>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    manage_board(pool.get_ref(), *path, auth.0.id).await?;
    let slug = body.slug.trim().to_ascii_lowercase();
    if slug.is_empty()
        || slug.len() > 64
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(AppError::Validation(
            "Category URL must contain lowercase letters, numbers or hyphens".into(),
        ));
    }
    valid_name_color(&body.name, &body.color)?;
    let category: BoardCategory = sqlx::query_as("INSERT INTO board_categories (board_id,slug,name,color) VALUES ($1,$2,$3,$4) RETURNING id,slug,name,color")
        .bind(*path).bind(slug).bind(body.name.trim()).bind(&body.color)
        .fetch_one(pool.get_ref()).await.map_err(|error| match &error {
            sqlx::Error::Database(db) if db.is_unique_violation() => AppError::Validation("Category URL already exists on this board".into()),
            _ => db_error(error),
        })?;
    Ok(HttpResponse::Created().json(category))
}

#[patch("/api/admin/boards/{board_id}/categories/{category_id}")]
pub async fn update(
    pool: web::Data<DbPool>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<CategoryUpdate>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let (board_id, category_id) = path.into_inner();
    manage_board(pool.get_ref(), board_id, auth.0.id).await?;
    valid_name_color(&body.name, &body.color)?;
    let category: Option<BoardCategory> = sqlx::query_as("UPDATE board_categories SET name=$1,color=$2 WHERE id=$3 AND board_id=$4 RETURNING id,slug,name,color")
        .bind(body.name.trim()).bind(&body.color).bind(category_id).bind(board_id)
        .fetch_optional(pool.get_ref()).await.map_err(db_error)?;
    Ok(HttpResponse::Ok().json(category.ok_or(AppError::NotFound)?))
}

#[delete("/api/admin/boards/{board_id}/categories/{category_id}")]
pub async fn remove(
    pool: web::Data<DbPool>,
    path: web::Path<(Uuid, Uuid)>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let (board_id, category_id) = path.into_inner();
    manage_board(pool.get_ref(), board_id, auth.0.id).await?;
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("UPDATE posts SET category_id=NULL WHERE board_id=$1 AND category_id=$2")
        .bind(board_id)
        .bind(category_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    let result = sqlx::query("DELETE FROM board_categories WHERE id=$1 AND board_id=$2")
        .bind(category_id)
        .bind(board_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
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
    async fn categories_are_board_scoped_and_search_filters_posts() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let own = seed_basic_tenant(&pool).await;
        let other = seed_basic_tenant(&pool).await;
        let own_board: uuid::Uuid = sqlx::query_scalar("SELECT id FROM boards WHERE slug=$1")
            .bind(&own.board_slug)
            .fetch_one(&pool)
            .await
            .unwrap();
        let own_admin = bearer_for(
            &own.admin_subject,
            &format!("{}@example.com", own.admin_subject),
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let other_admin = bearer_for(
            &other.admin_subject,
            &format!("{}@example.com", other.admin_subject),
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
        let url = format!("/api/admin/boards/{own_board}/categories");
        let denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&url)
                .insert_header(("Authorization", other_admin))
                .set_json(json!({"slug":"interface","name":"Interface","color":"#225588"}))
                .to_request(),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        let created = read_json(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&url)
                    .insert_header(("Authorization", own_admin))
                    .set_json(json!({"slug":"interface","name":"Interface","color":"#225588"}))
                    .to_request(),
            )
            .await,
        )
        .await;
        let category_id: uuid::Uuid = created["id"].as_str().unwrap().parse().unwrap();
        sqlx::query("UPDATE posts SET category_id=$1 WHERE id=$2")
            .bind(category_id)
            .bind(own.canonical_post_id)
            .execute(&pool)
            .await
            .unwrap();
        let public = read_json(
            test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!(
                        "/api/boards/{}/categories?tenant_slug={}",
                        own.board_slug, own.tenant_slug
                    ))
                    .to_request(),
            )
            .await,
        )
        .await;
        assert_eq!(public.as_array().unwrap().len(), 1);
        let updated = read_json(test::call_service(&app, test::TestRequest::patch()
            .uri(&format!("/api/admin/boards/{own_board}"))
            .insert_header(("Authorization", bearer_for(&own.admin_subject, &format!("{}@example.com", own.admin_subject), "Admin", "dev-secret")))
            .set_json(json!({"name":"Features","description":"Ideas","board_type":"feature-requests","is_private":false,
                "header_image_url":"https://images.example.com/cover.webp","background_image_url":"https://images.example.com/background.webp"}))
            .to_request()).await).await;
        assert_eq!(
            updated["header_image_url"],
            "https://images.example.com/cover.webp"
        );
        assert_eq!(
            updated["background_image_url"],
            "https://images.example.com/background.webp"
        );
        let filtered = read_json(
            test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!(
                        "/api/boards/{}/posts?tenant_slug={}&category=interface&q=Dark",
                        own.board_slug, own.tenant_slug
                    ))
                    .to_request(),
            )
            .await,
        )
        .await;
        assert_eq!(filtered["total"], 1);
        assert_eq!(filtered["items"][0]["category_name"], "Interface");
        assert_eq!(filtered["items"][0]["title"], "Dark Mode");
        let unrelated = read_json(
            test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!(
                        "/api/boards/{}/posts?tenant_slug={}&category=interface",
                        other.board_slug, other.tenant_slug
                    ))
                    .to_request(),
            )
            .await,
        )
        .await;
        assert_eq!(unrelated["total"], 0);
        let deleted = test::call_service(&app, test::TestRequest::delete()
            .uri(&format!("/api/admin/boards/{own_board}"))
            .insert_header(("Authorization", bearer_for(&own.admin_subject, &format!("{}@example.com", own.admin_subject), "Admin", "dev-secret")))
            .to_request()).await;
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    }
}
