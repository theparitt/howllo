use actix_web::{get, patch, post, web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;

use crate::audit::{self, AuditEntry};
use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::repositories::ai_suggestion_repository;

#[derive(Debug, Clone)]
pub struct DuplicateInput {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateSuggestion {
    pub post_id: String,
    pub score: f32,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct SummaryInput {
    pub post_title: String,
    pub comments: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThreadSummary {
    pub summary: String,
}

pub trait AiAssistant: Send + Sync {
    fn suggest_duplicates(
        &self,
        input: DuplicateInput,
    ) -> impl std::future::Future<Output = Result<Vec<DuplicateSuggestion>, AppError>> + Send;

    fn summarize_thread(
        &self,
        input: SummaryInput,
    ) -> impl std::future::Future<Output = Result<ThreadSummary, AppError>> + Send;
}

#[derive(Debug, Clone)]
pub struct LocalAiAssistant {
    pub settings: crate::config::AiSettings,
}

impl LocalAiAssistant {
    pub fn new(settings: crate::config::AiSettings) -> Self {
        Self { settings }
    }
}

impl AiAssistant for LocalAiAssistant {
    async fn suggest_duplicates(
        &self,
        input: DuplicateInput,
    ) -> Result<Vec<DuplicateSuggestion>, AppError> {
        let score = ((input.title.len() + input.body.len()) as f32 / 500.0).min(0.95);
        Ok(vec![DuplicateSuggestion {
            post_id: "heuristic".to_string(),
            score,
            reason: format!(
                "Heuristic similarity estimate from title/body using model {}",
                self.settings.model
            ),
        }])
    }

    async fn summarize_thread(&self, input: SummaryInput) -> Result<ThreadSummary, AppError> {
        let sample = input
            .comments
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        Ok(ThreadSummary {
            summary: format!("{}: {}", input.post_title, sample),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct AiTenantQuery {
    pub tenant_slug: String,
}

#[derive(Debug, Deserialize)]
pub struct ReviewAiSuggestionRequest {
    pub status: String,
}

async fn require_ai_admin_by_tenant_slug(
    pool: &DbPool,
    tenant_slug: &str,
    user_id: uuid::Uuid,
) -> Result<uuid::Uuid, AppError> {
    let tenant_id =
        crate::repositories::membership_repository::resolve_tenant_id(pool, tenant_slug).await?;

    crate::auth::require_permission(
        pool,
        tenant_id,
        user_id,
        crate::domain::permission::Permission::ModerateContent,
    )
    .await
    .map(|ctx| ctx.tenant_id)
}

#[get("/api/admin/ai/suggestions")]
pub async fn list_ai_suggestions(
    pool: web::Data<DbPool>,
    query: web::Query<AiTenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id =
        require_ai_admin_by_tenant_slug(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;

    let items = ai_suggestion_repository::list_ai_suggestions(pool.get_ref(), tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, tenant_id = %tenant_id, "error listing ai suggestions");
            AppError::InternalServerError
        })?;

    Ok(HttpResponse::Ok().json(items))
}

#[post("/api/admin/ai/posts/{post_id}/duplicate-suggestions")]
pub async fn create_duplicate_suggestion(
    pool: web::Data<DbPool>,
    settings: web::Data<crate::config::Settings>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<AiTenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    let tenant_id =
        require_ai_admin_by_tenant_slug(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;

    if !settings.ai.enabled {
        return Ok(HttpResponse::Ok().json(Vec::<ai_suggestion_repository::AiSuggestionRow>::new()));
    }

    let source = ai_suggestion_repository::get_post_source(pool.get_ref(), post_id, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, post_id = %post_id, tenant_id = %tenant_id, "error fetching source post for ai duplicate suggestion");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let assistant = LocalAiAssistant::new(settings.ai.clone());
    let heuristic = assistant
        .suggest_duplicates(DuplicateInput {
            title: source.0.clone(),
            body: source.1.clone(),
        })
        .await?;

    let payload = json!({
        "source_post_id": post_id,
        "suggestions": heuristic,
    });

    let item = ai_suggestion_repository::insert_ai_suggestion(
        pool.get_ref(),
        tenant_id,
        post_id,
        "duplicate",
        payload,
        auth.0.id,
    )
    .await
    .map_err(|error| {
        tracing::error!(error = %error, post_id = %post_id, tenant_id = %tenant_id, "error inserting ai duplicate suggestion");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Created().json(item))
}

#[post("/api/admin/ai/posts/{post_id}/summary")]
pub async fn create_thread_summary(
    pool: web::Data<DbPool>,
    settings: web::Data<crate::config::Settings>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<AiTenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let post_id = path.into_inner();
    let tenant_id =
        require_ai_admin_by_tenant_slug(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;

    if !settings.ai.enabled {
        return Ok(HttpResponse::Ok().json(Vec::<ai_suggestion_repository::AiSuggestionRow>::new()));
    }

    let title = ai_suggestion_repository::get_post_title(pool.get_ref(), post_id, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, post_id = %post_id, tenant_id = %tenant_id, "error fetching source post for ai summary");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)?;

    let comments =
        ai_suggestion_repository::get_thread_comments(pool.get_ref(), post_id)
            .await
            .map_err(|error| {
                tracing::error!(error = %error, post_id = %post_id, "error fetching comments for ai summary");
                AppError::InternalServerError
            })?;

    let assistant = LocalAiAssistant::new(settings.ai.clone());
    let summary = assistant
        .summarize_thread(SummaryInput {
            post_title: title,
            comments,
        })
        .await?;

    let item = ai_suggestion_repository::insert_ai_suggestion(
        pool.get_ref(),
        tenant_id,
        post_id,
        "summary",
        json!({ "summary": summary.summary }),
        auth.0.id,
    )
    .await
    .map_err(|error| {
        tracing::error!(error = %error, post_id = %post_id, tenant_id = %tenant_id, "error inserting ai summary suggestion");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Created().json(item))
}

#[post("/api/admin/ai/posts/{post_id}/tag-suggestions")]
pub async fn create_tag_suggestion(
    pool: web::Data<DbPool>,
    settings: web::Data<crate::config::Settings>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<AiTenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id =
        require_ai_admin_by_tenant_slug(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    if !settings.ai.enabled {
        return Ok(HttpResponse::Ok().json(Vec::<ai_suggestion_repository::AiSuggestionRow>::new()));
    }

    let post_id = path.into_inner();

    let title = ai_suggestion_repository::get_post_title(pool.get_ref(), post_id, tenant_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "error fetching post for tag suggestion");
            AppError::InternalServerError
        })?
        .unwrap_or_default();

    let suggested_tags = if title.to_lowercase().contains("dark") {
        vec!["ui", "accessibility"]
    } else if title.to_lowercase().contains("bug") {
        vec!["bug", "needs-triage"]
    } else {
        vec!["general"]
    };

    let item = ai_suggestion_repository::insert_ai_suggestion(
        pool.get_ref(),
        tenant_id,
        post_id,
        "suggested_tags",
        json!({ "tags": suggested_tags }),
        auth.0.id,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "error inserting ai tag suggestion");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Created().json(item))
}

#[post("/api/admin/ai/posts/{post_id}/moderation-suggestion")]
pub async fn create_moderation_suggestion(
    pool: web::Data<DbPool>,
    settings: web::Data<crate::config::Settings>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<AiTenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id =
        require_ai_admin_by_tenant_slug(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    if !settings.ai.enabled {
        return Ok(HttpResponse::Ok().json(Vec::<ai_suggestion_repository::AiSuggestionRow>::new()));
    }

    let item = ai_suggestion_repository::insert_ai_suggestion(
        pool.get_ref(),
        tenant_id,
        path.into_inner(),
        "moderation_flag",
        json!({ "suggested_action": "review_for_duplicates", "confidence": 0.5 }),
        auth.0.id,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "error inserting ai moderation suggestion");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Created().json(item))
}

#[post("/api/admin/ai/posts/{post_id}/grouping-suggestion")]
pub async fn create_grouping_suggestion(
    pool: web::Data<DbPool>,
    settings: web::Data<crate::config::Settings>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<AiTenantQuery>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id =
        require_ai_admin_by_tenant_slug(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;
    if !settings.ai.enabled {
        return Ok(HttpResponse::Ok().json(Vec::<ai_suggestion_repository::AiSuggestionRow>::new()));
    }

    let item = ai_suggestion_repository::insert_ai_suggestion(
        pool.get_ref(),
        tenant_id,
        path.into_inner(),
        "roadmap_grouping",
        json!({ "suggested_group": "ui-improvements" }),
        auth.0.id,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "error inserting ai grouping suggestion");
        AppError::InternalServerError
    })?;

    Ok(HttpResponse::Created().json(item))
}

#[patch("/api/admin/ai/suggestions/{suggestion_id}")]
pub async fn review_ai_suggestion(
    pool: web::Data<DbPool>,
    path: web::Path<uuid::Uuid>,
    query: web::Query<AiTenantQuery>,
    body: web::Json<ReviewAiSuggestionRequest>,
    auth: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let tenant_id =
        require_ai_admin_by_tenant_slug(pool.get_ref(), &query.tenant_slug, auth.0.id).await?;

    if !matches!(body.status.as_str(), "accepted" | "rejected") {
        return Err(AppError::Validation(
            "status must be accepted or rejected".to_string(),
        ));
    }

    let suggestion_id = path.into_inner();

    ai_suggestion_repository::update_ai_suggestion_status(
        pool.get_ref(),
        suggestion_id,
        tenant_id,
        &body.status,
        auth.0.id,
    )
    .await
    .map_err(|error| {
        tracing::error!(error = %error, tenant_id = %tenant_id, user_id = %auth.0.id, "error reviewing ai suggestion");
        AppError::InternalServerError
    })?;

    let _ = audit::record(
        pool.get_ref(),
        AuditEntry {
            tenant_id,
            actor_user_id: auth.0.id,
            entity_type: "ai_suggestion",
            entity_id: suggestion_id,
            action: audit::AI_SUGGESTION_REVIEWED,
            old_value: None,
            new_value: Some(json!({ "status": body.status })),
            reason: None,
        },
    )
    .await;

    Ok(HttpResponse::Ok().finish())
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
    async fn ai_disabled_returns_no_suggestions() {
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

        let request = test::TestRequest::post()
            .uri(&format!(
                "/api/admin/ai/posts/{}/duplicate-suggestions?tenant_slug={}",
                seed.canonical_post_id, seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert_eq!(body.as_array().unwrap().len(), 0);
    }

    #[actix_web::test]
    async fn ai_enabled_creates_reviewable_suggestion() {
        let mut settings = test_settings();
        settings.ai.enabled = true;
        settings.ai.provider = crate::config::AiProvider::Local;

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
                "/api/admin/ai/posts/{}/summary?tenant_slug={}",
                seed.canonical_post_id, seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .to_request();
        let create_response = test::call_service(&app, create_request).await;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = read_json(create_response).await;
        let suggestion_id = created.get("id").and_then(|value| value.as_str()).unwrap();

        let review_request = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/ai/suggestions/{suggestion_id}?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .set_json(json!({ "status": "accepted" }))
            .to_request();
        let review_response = test::call_service(&app, review_request).await;
        assert_eq!(review_response.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn ai_cannot_directly_mutate_post() {
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

        let suggestion_request = test::TestRequest::post()
            .uri(&format!(
                "/api/admin/ai/posts/{}/summary?tenant_slug={}",
                seed.canonical_post_id, seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let resp = test::call_service(&app, suggestion_request).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let post_after = sqlx::query!(
            "SELECT status, is_hidden, deleted_at FROM posts WHERE id = $1",
            seed.canonical_post_id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(post_after.status, "planned");
        assert!(!post_after.is_hidden);
        assert!(post_after.deleted_at.is_none());
    }

    #[actix_web::test]
    async fn ai_suggestions_are_tenant_scoped() {
        let settings = test_settings();
        let _guard = lock_test_db().await;
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let seed = seed_basic_tenant(&pool).await;

        let tenant_b_id = uuid::Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO tenants (id, slug, name) VALUES ($1, 'ai-tenant-b', 'B')",
            tenant_b_id
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO ai_suggestions (tenant_id, post_id, suggestion_type, payload, created_by_user_id) VALUES ($1, $2, 'duplicate', '{}'::jsonb, $3)",
            tenant_b_id,
            seed.canonical_post_id,
            seed.admin_user_id,
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

        let token = bearer_for(
            &seed.admin_subject,
            "admin@example.com",
            "Admin",
            &settings.rooiam_jwt_secret,
        );

        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/ai/suggestions?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        let items = body.as_array().unwrap();
        assert!(items.is_empty());
    }

    #[actix_web::test]
    async fn accepted_ai_suggestion_creates_audit_log() {
        let mut settings = test_settings();
        settings.ai.enabled = true;
        settings.ai.provider = crate::config::AiProvider::Local;

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

        let create = test::TestRequest::post()
            .uri(&format!(
                "/api/admin/ai/posts/{}/summary?tenant_slug={}",
                seed.canonical_post_id, seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .to_request();
        let resp = test::call_service(&app, create).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let created = read_json(resp).await;
        let sid = created.get("id").and_then(|v| v.as_str()).unwrap();

        let review = test::TestRequest::patch()
            .uri(&format!(
                "/api/admin/ai/suggestions/{sid}?tenant_slug={}",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token.clone()))
            .set_json(json!({ "status": "accepted" }))
            .to_request();
        let resp = test::call_service(&app, review).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let audit_req = test::TestRequest::get()
            .uri(&format!(
                "/api/admin/audit-logs?tenant_slug={}&page=1&per_page=10",
                seed.tenant_slug
            ))
            .insert_header(("Authorization", token))
            .to_request();
        let audit_resp = test::call_service(&app, audit_req).await;
        assert_eq!(audit_resp.status(), StatusCode::OK);
        let audit_body = read_json(audit_resp).await;
        let items = audit_body.get("items").and_then(|v| v.as_array()).unwrap();
        let has_ai_audit = items.iter().any(|item| {
            item.get("action").and_then(|v| v.as_str()) == Some("ai_suggestion_reviewed")
        });
        assert!(has_ai_audit);
    }
}
