use sqlx::QueryBuilder;

use crate::db::DbPool;
use crate::dto::PostListItemDto;

pub async fn fetch_roadmap_items(
    pool: &DbPool,
    tenant_slug: &str,
    member_user_id: Option<uuid::Uuid>,
    status_filter: Option<&str>,
    tag_filter: Option<&str>,
    board_slug_filter: Option<&str>,
) -> Result<Vec<PostListItemDto>, sqlx::Error> {
    let mut builder = QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT p.id, p.title, p.status, p.vote_count,
               (SELECT count(*) FROM comments c WHERE c.post_id = p.id AND c.is_hidden = false) AS comment_count,
               p.duplicate_of_post_id, p.created_at, p.is_hidden, p.deleted_at
        FROM posts p
        JOIN boards b ON p.board_id = b.id
        JOIN tenants t ON p.tenant_id = t.id
        "#,
    );

    if tag_filter.is_some() {
        builder
            .push(" JOIN post_tags pt ON pt.post_id = p.id JOIN tags tag ON tag.id = pt.tag_id ");
    }

    builder.push(" WHERE t.slug = ");
    builder.push_bind(tenant_slug);
    builder.push(" AND p.is_hidden = false AND p.deleted_at IS NULL ");
    builder.push(" AND p.status IN ('planned', 'in_progress', 'done') ");
    builder.push(" AND p.duplicate_of_post_id IS NULL ");

    match member_user_id {
        Some(user_id) => {
            builder.push(" AND (b.is_private = false OR EXISTS (SELECT 1 FROM memberships m WHERE m.tenant_id = t.id AND m.user_id = ");
            builder.push_bind(user_id);
            builder.push(" AND m.public_participant = FALSE) OR EXISTS (SELECT 1 FROM tenants t2 JOIN account_memberships am ON am.account_id = t2.account_id WHERE t2.id = t.id AND am.user_id = ");
            builder.push_bind(user_id);
            builder.push(" AND am.role IN ('owner', 'admin'))) ");
        }
        None => {
            builder.push(" AND b.is_private = false ");
        }
    }

    if let Some(status) = status_filter {
        builder.push(" AND p.status = ");
        builder.push_bind(status);
    }

    if let Some(tag) = tag_filter {
        builder.push(" AND tag.slug = ");
        builder.push_bind(tag.trim());
    }

    if let Some(board_slug) = board_slug_filter {
        builder.push(" AND b.slug = ");
        builder.push_bind(board_slug.trim());
    }

    builder.push(" ORDER BY CASE p.status WHEN 'in_progress' THEN 1 WHEN 'planned' THEN 2 ELSE 3 END, p.created_at DESC");

    builder
        .build_query_as::<PostListItemDto>()
        .fetch_all(pool)
        .await
}
