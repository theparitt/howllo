use crate::db::DbPool;
use actix_web::{get, web, HttpResponse, Responder};

use crate::{
    ai, api_tokens, audit, auth, boards, comments, exports, invitations, me, memberships,
    moderation, moderation_notes, notifications, platform, posts, realtime, search, subscriptions,
    tags, tenancy, votes, webhooks,
};

#[get("/api/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

#[get("/api/ready")]
async fn readiness_check(
    pool: web::Data<DbPool>,
) -> Result<impl Responder, crate::errors::AppError> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool.get_ref())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "database readiness check failed");
            crate::errors::AppError::InternalServerError
        })?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "ready" })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check)
        .service(readiness_check)
        .service(auth::local_admin::auth_state)
        .service(auth::local_admin::auth_setup)
        .service(auth::local_admin::auth_login)
        .service(auth::create_workspace_session)
        .service(auth::revoke_workspace_session)
        .service(platform::get_platform_status)
        .service(platform::get_build_info)
        .service(platform::get_storage_config)
        .service(platform::get_storage_usage)
        .service(platform::save_storage_config)
        .service(platform::test_storage)
        .service(platform::test_database)
        .service(platform::upload_image)
        .service(platform::serve_upload)
        .service(tenancy::get_bootstrap_tenant)
        .service(tenancy::list_admin_tenants)
        .service(tenancy::create_admin_tenant)
        .service(tenancy::delete_admin_tenant)
        .service(tenancy::get_tenant_branding)
        .service(tenancy::get_workspace_auth_config)
        .service(tenancy::update_tenant_branding)
        .service(tenancy::update_workspace_auth_config)
        .service(realtime::handler::ws_connect)
        .service(boards::api::list_boards)
        .service(boards::api::get_board_detail)
        .service(boards::api::list_admin_boards)
        .service(boards::api::create_board)
        .service(boards::api::update_board)
        .service(boards::api::delete_board)
        .service(memberships::list_members)
        .service(memberships::create_member)
        .service(memberships::update_member_role)
        .service(memberships::remove_member)
        .service(invitations::create_invitation)
        .service(invitations::list_invitations)
        .service(invitations::withdraw_invitation)
        .service(invitations::list_my_invitations)
        .service(invitations::accept_invitation)
        .service(invitations::reject_invitation)
        .service(me::get_me)
        .service(me::update_me)
        .service(me::get_me_activity)
        .service(posts::api::list_board_posts)
        .service(posts::api::get_post_detail)
        .service(posts::api::get_post_status_history)
        .service(posts::api::create_post)
        .service(posts::api::update_post)
        .service(posts::api::get_roadmap)
        .service(posts::api::get_roadmap_by_status)
        .service(posts::api::get_roadmap_by_tag)
        .service(posts::api::get_roadmap_grouped)
        .service(posts::api::get_post_activity)
        .service(boards::api::get_board_summary)
        .service(tags::api::get_tag_summary)
        .service(search::search_posts)
        .service(search::suggest_duplicates)
        .service(ai::list_ai_suggestions)
        .service(ai::create_duplicate_suggestion)
        .service(ai::create_thread_summary)
        .service(ai::create_tag_suggestion)
        .service(ai::create_moderation_suggestion)
        .service(ai::create_grouping_suggestion)
        .service(ai::review_ai_suggestion)
        .service(notifications::list_notifications)
        .service(notifications::mark_notification_read)
        .service(tags::api::list_tags)
        .service(comments::api::create_comment)
        .service(comments::api::list_comments)
        .service(votes::api::add_vote)
        .service(votes::api::remove_vote)
        .service(subscriptions::api::follow_post)
        .service(subscriptions::api::unfollow_post)
        .service(subscriptions::api::get_follow_state)
        .service(audit::list_audit_logs)
        .service(tags::api::create_tag)
        .service(tags::api::update_tag)
        .service(tags::api::delete_tag)
        .service(tags::api::attach_tag_to_post)
        .service(tags::api::detach_tag_from_post)
        .service(moderation_notes::create_post_note)
        .service(moderation_notes::list_post_notes)
        .service(moderation_notes::create_comment_note)
        .service(moderation_notes::list_comment_notes)
        .service(webhooks::list_webhooks)
        .service(webhooks::create_webhook)
        .service(webhooks::deactivate_webhook)
        .service(webhooks::get_delivery_status)
        .service(webhooks::resend_webhook_delivery)
        .service(api_tokens::list_api_tokens)
        .service(api_tokens::create_api_token)
        .service(api_tokens::revoke_api_token)
        .service(exports::export_posts_json)
        .service(exports::export_posts_csv)
        .service(moderation::api::update_post_status)
        .service(moderation::api::update_post_duplicate)
        .service(moderation::api::update_post_visibility)
        .service(moderation::api::soft_delete_post)
        .service(moderation::api::restore_post)
        .service(moderation::api::update_post_lock)
        .service(moderation::api::update_comment_official)
        .service(moderation::api::update_comment_visibility)
        .service(moderation::api::get_moderation_queue);
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, App};

    use super::configure;

    #[actix_web::test]
    async fn health_endpoint_is_available() {
        let app = test::init_service(App::new().configure(configure)).await;
        let request = test::TestRequest::get().uri("/api/health").to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::OK);
    }
}
