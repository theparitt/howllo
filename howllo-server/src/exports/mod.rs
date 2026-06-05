use actix_web::{get, web, HttpResponse, Responder};

use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::services::export_service;

#[derive(Debug, serde::Deserialize)]
pub struct ExportQuery {
    pub tenant_slug: String,
}

#[get("/api/admin/export/posts.json")]
pub async fn export_posts_json(
    pool: web::Data<DbPool>,
    query: web::Query<ExportQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let posts =
        export_service::export_posts_json(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    Ok(HttpResponse::Ok().json(posts))
}

#[get("/api/admin/export/posts.csv")]
pub async fn export_posts_csv(
    pool: web::Data<DbPool>,
    query: web::Query<ExportQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let posts =
        export_service::export_posts_csv(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;

    let mut writer = csv::Writer::from_writer(vec![]);
    for post in posts {
        writer.serialize(post).map_err(|e| {
            tracing::error!(error = %e, "error writing export csv");
            AppError::InternalServerError
        })?;
    }
    let bytes = writer.into_inner().map_err(|e| {
        tracing::error!(error = %e, "error finalizing export csv");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Ok()
        .content_type("text/csv; charset=utf-8")
        .body(bytes))
}

#[cfg(test)]
mod tests;
