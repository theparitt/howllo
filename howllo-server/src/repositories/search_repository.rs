use sqlx::QueryBuilder;
use uuid::Uuid;

use crate::db::DbPool;
use crate::dto::PostListItemDto;

pub async fn search_posts(
    pool: &DbPool,
    tenant_slug: &str,
    query: &str,
    sort: &str,
    board_slug: Option<&str>,
    tag_slug: Option<&str>,
    status_filter: Option<&str>,
    member_user_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<PostListItemDto>, sqlx::Error> {
    let clean_query = query.trim().to_lowercase();
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

    let use_fts = !clean_query.is_empty();
    if use_fts {
        builder.push(", ts_rank(p.search_vector, plainto_tsquery('english', ");
        builder.push_bind(clean_query.as_str());
        builder.push(")) AS rank ");
    }

    if tag_slug.is_some() {
        builder
            .push(" JOIN post_tags pt ON pt.post_id = p.id JOIN tags tag ON tag.id = pt.tag_id ");
    }

    builder.push(" WHERE t.slug = ");
    builder.push_bind(tenant_slug);
    builder.push(" AND p.is_hidden = false AND p.deleted_at IS NULL ");

    if use_fts {
        builder.push(" AND p.search_vector @@ plainto_tsquery('english', ");
        builder.push_bind(clean_query.as_str());
        builder.push(") ");
    }

    if let Some(board_slug) = board_slug {
        builder.push(" AND b.slug = ");
        builder.push_bind(board_slug);
    }

    if let Some(tag_slug) = tag_slug {
        builder.push(" AND tag.slug = ");
        builder.push_bind(tag_slug.trim());
    }

    if let Some(status) = status_filter {
        builder.push(" AND p.status = ");
        builder.push_bind(status);
    }

    match member_user_id {
        Some(user_id) => {
            builder.push(" AND (b.is_private = false OR EXISTS (SELECT 1 FROM memberships m WHERE m.tenant_id = t.id AND m.user_id = ");
            builder.push_bind(user_id);
            builder.push(")) ");
        }
        None => {
            builder.push(" AND b.is_private = false ");
        }
    }

    builder.push(" ORDER BY ");
    match sort {
        "newest" => builder.push("p.created_at DESC"),
        "top" | "most_voted" => builder.push("p.vote_count DESC, p.created_at DESC"),
        "most_commented" => builder.push("comment_count DESC, p.created_at DESC"),
        "relevance" if use_fts => builder.push("rank DESC, p.created_at DESC"),
        _ if use_fts => builder.push("rank DESC, p.created_at DESC"),
        _ => builder.push("p.updated_at DESC, p.created_at DESC"),
    };

    builder.push(" LIMIT ");
    builder.push_bind(limit);

    builder
        .build_query_as::<PostListItemDto>()
        .fetch_all(pool)
        .await
}
