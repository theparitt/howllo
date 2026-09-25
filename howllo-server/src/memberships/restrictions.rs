use actix_web::{delete, get, put, web, HttpResponse, Responder};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{require_permission, AuthenticatedUser};
use crate::db::DbPool;
use crate::domain::permission::Permission;
use crate::errors::AppError;
use crate::repositories::membership_repository::resolve_tenant_id;

#[derive(Deserialize)]
pub struct WorkspaceQuery {
    pub tenant_slug: String,
}

#[derive(Deserialize)]
pub struct RestrictionInput {
    pub kind: String,
    pub duration_days: Option<i64>,
    pub reason: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Participant {
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,
    pub joined_at: DateTime<Utc>,
    pub restriction_kind: Option<String>,
    pub restriction_reason: Option<String>,
    pub restriction_expires_at: Option<DateTime<Utc>>,
}

#[get("/api/admin/participants")]
pub async fn list_participants(
    pool: web::Data<DbPool>,
    query: web::Query<WorkspaceQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = resolve_tenant_id(pool.get_ref(), &query.tenant_slug).await?;
    require_permission(
        pool.get_ref(),
        tenant_id,
        auth.0.id,
        Permission::ManageMembers,
    )
    .await?;
    let rows = sqlx::query_as::<_, Participant>(
        r#"SELECT m.user_id, u.display_name, u.email, m.created_at AS joined_at,
                  CASE WHEN r.expires_at IS NULL OR r.expires_at > NOW() THEN r.kind END AS restriction_kind,
                  CASE WHEN r.expires_at IS NULL OR r.expires_at > NOW() THEN r.reason END AS restriction_reason,
                  CASE WHEN r.expires_at IS NULL OR r.expires_at > NOW() THEN r.expires_at END AS restriction_expires_at
           FROM memberships m
           JOIN users u ON u.id = m.user_id
           LEFT JOIN workspace_restrictions r ON r.tenant_id = m.tenant_id AND r.user_id = m.user_id
           WHERE m.tenant_id = $1 AND m.public_participant = TRUE
           ORDER BY m.created_at DESC"#,
    )
    .bind(tenant_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(%error, %tenant_id, "error listing workspace participants");
        AppError::InternalServerError
    })?;
    Ok(HttpResponse::Ok().json(rows))
}

#[put("/api/admin/participants/{user_id}/restriction")]
pub async fn set_restriction(
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    query: web::Query<WorkspaceQuery>,
    body: web::Json<RestrictionInput>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = resolve_tenant_id(pool.get_ref(), &query.tenant_slug).await?;
    require_permission(
        pool.get_ref(),
        tenant_id,
        auth.0.id,
        Permission::ManageMembers,
    )
    .await?;
    let user_id = path.into_inner();
    if user_id == auth.0.id {
        return Err(AppError::Validation("cannot restrict yourself".into()));
    }
    let reason = body.reason.trim();
    if reason.is_empty() || reason.len() > 500 {
        return Err(AppError::Validation(
            "reason must be 1–500 characters".into(),
        ));
    }
    let expires_at = match body.kind.as_str() {
        "suspended" => {
            let days = body
                .duration_days
                .ok_or_else(|| AppError::Validation("duration_days is required".into()))?;
            if !(1..=365).contains(&days) {
                return Err(AppError::Validation("duration_days must be 1–365".into()));
            }
            Some(Utc::now() + Duration::days(days))
        }
        "banned" if body.duration_days.is_none() => None,
        "banned" => return Err(AppError::Validation("bans cannot have a duration".into())),
        _ => {
            return Err(AppError::Validation(
                "kind must be suspended or banned".into(),
            ))
        }
    };
    // Only public participants can be restricted here. Staff must be managed
    // through the separate invitation and staff membership flow.
    let is_participant: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM memberships m WHERE m.tenant_id = $1 AND m.user_id = $2 AND m.public_participant = TRUE AND NOT EXISTS (SELECT 1 FROM tenants t JOIN account_memberships am ON am.account_id = t.account_id WHERE t.id = m.tenant_id AND am.user_id = m.user_id AND am.role IN ('owner', 'admin')))",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|error| { tracing::error!(%error, "error checking participant"); AppError::InternalServerError })?;
    if !is_participant {
        return Err(AppError::NotFound);
    }
    sqlx::query(
        r#"INSERT INTO workspace_restrictions (tenant_id, user_id, kind, reason, expires_at, created_by)
           VALUES ($1, $2, $3, $4, $5, $6)
           ON CONFLICT (tenant_id, user_id) DO UPDATE SET
             kind = EXCLUDED.kind, reason = EXCLUDED.reason,
             expires_at = EXCLUDED.expires_at, created_by = EXCLUDED.created_by,
             updated_at = NOW()"#,
    )
    .bind(tenant_id).bind(user_id).bind(&body.kind).bind(reason).bind(expires_at).bind(auth.0.id)
    .execute(pool.get_ref()).await
    .map_err(|error| { tracing::error!(%error, "error restricting participant"); AppError::InternalServerError })?;
    Ok(HttpResponse::NoContent().finish())
}

#[delete("/api/admin/participants/{user_id}/restriction")]
pub async fn clear_restriction(
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    query: web::Query<WorkspaceQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id = resolve_tenant_id(pool.get_ref(), &query.tenant_slug).await?;
    require_permission(
        pool.get_ref(),
        tenant_id,
        auth.0.id,
        Permission::ManageMembers,
    )
    .await?;
    sqlx::query("DELETE FROM workspace_restrictions WHERE tenant_id = $1 AND user_id = $2")
        .bind(tenant_id)
        .bind(path.into_inner())
        .execute(pool.get_ref())
        .await
        .map_err(|error| {
            tracing::error!(%error, "error clearing participant restriction");
            AppError::InternalServerError
        })?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::json;

    use crate::db;
    use crate::http::test_support::{
        bearer_for, lock_test_db, read_json, reset_db, seed_basic_tenant, test_settings,
    };
    use crate::startup;

    #[actix_web::test]
    async fn suspension_blocks_existing_session_and_expires() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;
        sqlx::query("UPDATE memberships SET public_participant = TRUE WHERE user_id = $1")
            .bind(seed.member_user_id)
            .execute(&pool)
            .await
            .unwrap();
        let other_tenant = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Other project')")
            .bind(other_tenant)
            .bind(format!("other-{other_tenant}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (tenant_id, user_id, role, public_participant) VALUES ($1, $2, 'member', TRUE)")
            .bind(other_tenant)
            .bind(seed.member_user_id)
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
        let admin = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );
        let member = bearer_for(
            &seed.member_subject,
            "member@example.com",
            "Member",
            &settings.rooiam_jwt_secret,
        );
        let session_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/workspace-session")
                .insert_header(("Authorization", member.clone()))
                .set_json(json!({ "tenant_slug": seed.tenant_slug }))
                .to_request(),
        )
        .await;
        assert_eq!(session_response.status(), StatusCode::CREATED);
        let session = read_json(session_response).await;
        let bearer = format!("Bearer {}", session["session_token"].as_str().unwrap());
        let restriction_url = format!(
            "/api/admin/participants/{}/restriction?tenant_slug={}",
            seed.member_user_id, seed.tenant_slug
        );
        let put = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&restriction_url)
                .insert_header(("Authorization", admin.clone()))
                .set_json(json!({ "kind": "suspended", "duration_days": 7, "reason": "Spam" }))
                .to_request(),
        )
        .await;
        assert_eq!(put.status(), StatusCode::NO_CONTENT);
        assert!(
            crate::memberships::check_membership(&pool, other_tenant, seed.member_user_id)
                .await
                .is_ok(),
            "another workspace remains available"
        );

        let vote_url = format!("/api/posts/{}/vote", seed.canonical_post_id);
        let blocked = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&vote_url)
                .insert_header(("Authorization", bearer.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(blocked.status(), StatusCode::UNAUTHORIZED);

        let second_session = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/workspace-session")
                .insert_header(("Authorization", member.clone()))
                .set_json(json!({ "tenant_slug": seed.tenant_slug }))
                .to_request(),
        )
        .await;
        assert_eq!(second_session.status(), StatusCode::FORBIDDEN);

        sqlx::query("UPDATE workspace_restrictions SET expires_at = NOW() - INTERVAL '1 second' WHERE user_id = $1")
            .bind(seed.member_user_id).execute(&pool).await.unwrap();
        let allowed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&vote_url)
                .insert_header(("Authorization", bearer))
                .to_request(),
        )
        .await;
        assert_eq!(allowed.status(), StatusCode::OK);

        let ban = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&restriction_url)
                .insert_header(("Authorization", admin.clone()))
                .set_json(json!({ "kind": "banned", "reason": "Repeated abuse" }))
                .to_request(),
        )
        .await;
        assert_eq!(ban.status(), StatusCode::NO_CONTENT);
        assert!(
            crate::memberships::check_membership(&pool, other_tenant, seed.member_user_id)
                .await
                .is_ok()
        );
        let lift = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&restriction_url)
                .insert_header(("Authorization", admin))
                .to_request(),
        )
        .await;
        assert_eq!(lift.status(), StatusCode::NO_CONTENT);
        let tenant_id =
            crate::repositories::membership_repository::resolve_tenant_id(&pool, &seed.tenant_slug)
                .await
                .unwrap();
        assert!(
            crate::memberships::check_membership(&pool, tenant_id, seed.member_user_id)
                .await
                .is_ok()
        );
    }
}
