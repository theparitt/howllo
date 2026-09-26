use chrono::{DateTime, Utc};
use sqlx::QueryBuilder;
use sqlx::{FromRow, Row};
use uuid::Uuid;

use crate::db::DbPool;
use crate::dto::{OfficialResponseSummaryDto, PostFollowStateDto, PostListItemDto, TagDto};

pub struct BoardAccessRecord {
    pub board_id: Uuid,
    pub tenant_id: Uuid,
    pub is_private: bool,
    pub board_type: String,
}

#[derive(FromRow)]
pub struct PostAccessRecord {
    pub tenant_id: Uuid,
    pub board_id: Uuid,
    pub is_private: bool,
    pub is_hidden: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub is_locked: bool,
    pub allow_votes: bool,
    pub allow_comments: bool,
}

#[derive(Debug)]
pub struct PostDetailRecord {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub status: String,
    pub author_display_name: String,
    pub vote_count: i32,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub duplicate_of_post_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub board_slug: String,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub attachments: Vec<String>,
}

pub struct PostStatusContextRecord {
    pub tenant_id: Uuid,
    pub status: String,
    pub title: String,
}

pub struct EditablePostRecord {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub is_hidden: bool,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub struct BoardPostListQuery<'a> {
    pub tenant_slug: &'a str,
    pub board_slug: &'a str,
    pub sort: &'a str,
    pub status: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub category: Option<&'a str>,
    pub q: Option<&'a str>,
    pub include_hidden: bool,
    pub limit: i64,
    pub offset: i64,
}

pub async fn find_board_access(
    pool: &DbPool,
    tenant_slug: &str,
    board_slug: &str,
) -> Result<Option<BoardAccessRecord>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT b.id, b.is_private, b.board_type, t.id as tenant_id
        FROM boards b
        JOIN tenants t ON b.tenant_id = t.id
        WHERE t.slug = $1 AND t.is_published = TRUE AND b.slug = $2 AND b.is_enabled = TRUE
        "#,
    )
    .bind(tenant_slug)
    .bind(board_slug)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| BoardAccessRecord {
        board_id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        is_private: row.get("is_private"),
        board_type: row.get("board_type"),
    }))
}

pub async fn find_post_access(
    pool: &DbPool,
    post_id: Uuid,
    tenant_slug: Option<&str>,
) -> Result<Option<PostAccessRecord>, sqlx::Error> {
    if let Some(tenant_slug) = tenant_slug {
        sqlx::query_as::<_, PostAccessRecord>(
            r#"
            SELECT p.tenant_id, p.board_id, p.is_hidden, p.deleted_at, p.is_locked, b.is_private, b.allow_votes, b.allow_comments
            FROM posts p
            JOIN boards b ON p.board_id = b.id
            JOIN tenants t ON p.tenant_id = t.id
            WHERE p.id = $1 AND t.slug = $2 AND t.is_published = TRUE AND b.is_enabled = TRUE
            "#,
        )
        .bind(post_id)
        .bind(tenant_slug)
        .fetch_optional(pool)
        .await
    } else {
        sqlx::query_as::<_, PostAccessRecord>(
            r#"
            SELECT p.tenant_id, p.board_id, p.is_hidden, p.deleted_at, p.is_locked, b.is_private, b.allow_votes, b.allow_comments
            FROM posts p
            JOIN boards b ON p.board_id = b.id
            JOIN tenants t ON p.tenant_id = t.id
            WHERE p.id = $1 AND t.is_published = TRUE AND b.is_enabled = TRUE
            "#,
        )
        .bind(post_id)
        .fetch_optional(pool)
        .await
    }
}

pub async fn count_board_posts(
    pool: &DbPool,
    tenant_slug: &str,
    board_slug: &str,
    status: Option<&str>,
    tag: Option<&str>,
    category: Option<&str>,
    q: Option<&str>,
    include_hidden: bool,
) -> Result<i64, sqlx::Error> {
    let mut builder = QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT count(*)::bigint AS count
        FROM posts p
        JOIN boards b ON p.board_id = b.id
        JOIN tenants t ON b.tenant_id = t.id
        "#,
    );

    if tag.is_some() {
        builder
            .push(" JOIN post_tags pt ON pt.post_id = p.id JOIN tags tag ON tag.id = pt.tag_id ");
    }

    builder.push(" WHERE t.slug = ");
    builder.push_bind(tenant_slug);
    builder.push(" AND b.slug = ");
    builder.push_bind(board_slug);

    if !include_hidden {
        builder.push(" AND p.is_hidden = false AND p.deleted_at IS NULL ");
    }

    if let Some(status) = status {
        builder.push(" AND p.status = ");
        builder.push_bind(status);
    }

    if let Some(tag) = tag {
        builder.push(" AND tag.slug = ");
        builder.push_bind(tag.trim());
    }
    if let Some(category) = category {
        builder.push(" AND EXISTS (SELECT 1 FROM board_categories cat WHERE cat.id=p.category_id AND cat.board_id=b.id AND cat.slug=");
        builder.push_bind(category.trim());
        builder.push(")");
    }
    if let Some(q) = q.filter(|q| !q.trim().is_empty()) {
        builder.push(" AND (p.title ILIKE ");
        builder.push_bind(format!("%{}%", q.trim()));
        builder.push(" OR p.body ILIKE ");
        builder.push_bind(format!("%{}%", q.trim()));
        builder.push(")");
    }

    let row: (i64,) = builder.build_query_as().fetch_one(pool).await?;
    Ok(row.0)
}

pub async fn list_board_posts(
    pool: &DbPool,
    query: BoardPostListQuery<'_>,
) -> Result<Vec<PostListItemDto>, sqlx::Error> {
    let mut builder = QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT p.id, p.title, p.status, cat.name AS category_name, cat.color AS category_color,
               ARRAY(SELECT tag_name.name FROM post_tags post_tag JOIN tags tag_name ON tag_name.id=post_tag.tag_id WHERE post_tag.post_id=p.id ORDER BY tag_name.name) AS tag_names,
               p.vote_count, (p.pinned_at IS NOT NULL) AS is_pinned,
               (SELECT count(*) FROM comments c WHERE c.post_id = p.id AND c.is_hidden = false) AS comment_count,
               p.duplicate_of_post_id, p.created_at, p.is_hidden, p.deleted_at
        FROM posts p
        JOIN boards b ON p.board_id = b.id
        JOIN tenants t ON b.tenant_id = t.id
        LEFT JOIN board_categories cat ON cat.id=p.category_id AND cat.board_id=b.id
        "#,
    );

    if query.tag.is_some() {
        builder
            .push(" JOIN post_tags pt ON pt.post_id = p.id JOIN tags tag ON tag.id = pt.tag_id ");
    }

    builder.push(" WHERE t.slug = ");
    builder.push_bind(query.tenant_slug);
    builder.push(" AND b.slug = ");
    builder.push_bind(query.board_slug);

    if !query.include_hidden {
        builder.push(" AND p.is_hidden = false AND p.deleted_at IS NULL ");
    }

    if let Some(status) = query.status {
        builder.push(" AND p.status = ");
        builder.push_bind(status);
    }

    if let Some(tag) = query.tag {
        builder.push(" AND tag.slug = ");
        builder.push_bind(tag.trim());
    }
    if let Some(category) = query.category {
        builder.push(" AND cat.slug = ");
        builder.push_bind(category.trim());
    }
    if let Some(q) = query.q.filter(|q| !q.trim().is_empty()) {
        builder.push(" AND (p.title ILIKE ");
        builder.push_bind(format!("%{}%", q.trim()));
        builder.push(" OR p.body ILIKE ");
        builder.push_bind(format!("%{}%", q.trim()));
        builder.push(")");
    }

    builder.push(" ORDER BY ");
    match query.sort {
        "top" => builder.push("p.vote_count DESC, p.created_at DESC"),
        "hot" => builder.push("(CASE WHEN p.created_at > NOW() - INTERVAL '7 days' THEN 1 ELSE 0 END) DESC, (p.vote_count * 2 + (SELECT count(*) FROM comments hot_c WHERE hot_c.post_id=p.id AND hot_c.is_hidden=FALSE)) DESC, p.created_at DESC"),
        "oldest" => builder.push("p.created_at ASC"),
        _ => builder.push("p.pinned_at DESC NULLS LAST, p.created_at DESC"),
    };

    builder.push(" LIMIT ");
    builder.push_bind(query.limit);
    builder.push(" OFFSET ");
    builder.push_bind(query.offset);

    builder
        .build_query_as::<PostListItemDto>()
        .fetch_all(pool)
        .await
}

pub async fn find_post_detail(
    pool: &DbPool,
    post_id: Uuid,
    tenant_slug: &str,
) -> Result<Option<PostDetailRecord>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT p.id, p.title, p.body, p.status, u.display_name as author_display_name,
               p.vote_count, (p.pinned_at IS NOT NULL) AS "is_pinned!", p.is_locked, p.duplicate_of_post_id, p.created_at, b.slug as board_slug,
               p.attachments, cat.name AS "category_name?", cat.color AS "category_color?"
        FROM posts p
        JOIN boards b ON p.board_id = b.id
        JOIN tenants t ON p.tenant_id = t.id
        JOIN users u ON p.user_id = u.id
        LEFT JOIN board_categories cat ON cat.id=p.category_id AND cat.board_id=b.id
        WHERE p.id = $1 AND t.slug = $2 AND p.is_hidden = false AND p.deleted_at IS NULL
        "#,
        post_id,
        tenant_slug
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| PostDetailRecord {
        id: row.id,
        title: row.title,
        body: row.body,
        status: row.status,
        author_display_name: row.author_display_name,
        vote_count: row.vote_count,
        is_pinned: row.is_pinned,
        is_locked: row.is_locked,
        duplicate_of_post_id: row.duplicate_of_post_id,
        created_at: row.created_at,
        board_slug: row.board_slug,
        category_name: row.category_name,
        category_color: row.category_color,
        attachments: serde_json::from_value(row.attachments).unwrap_or_default(),
    }))
}

pub async fn get_post_tags(pool: &DbPool, post_id: Uuid) -> Result<Vec<TagDto>, sqlx::Error> {
    sqlx::query_as!(
        TagDto,
        r#"
        SELECT t.id, t.slug, t.name, t.color
        FROM post_tags pt
        JOIN tags t ON t.id = pt.tag_id
        WHERE pt.post_id = $1
        ORDER BY t.name ASC
        "#,
        post_id
    )
    .fetch_all(pool)
    .await
}

pub async fn get_visible_comment_count(pool: &DbPool, post_id: Uuid) -> Result<i64, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT count(*)::bigint AS count FROM comments WHERE post_id = $1 AND is_hidden = false",
        post_id
    )
    .fetch_one(pool)
    .await?;

    Ok(row.count.unwrap_or(0))
}

pub async fn get_follow_state(
    pool: &DbPool,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<Option<PostFollowStateDto>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT notify_on_status_change, notify_on_official_response
        FROM post_follows
        WHERE post_id = $1 AND user_id = $2
        "#,
        post_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| PostFollowStateDto {
        is_following: true,
        notify_on_status_change: row.notify_on_status_change,
        notify_on_official_response: row.notify_on_official_response,
    }))
}

pub async fn get_official_response(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Option<OfficialResponseSummaryDto>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT c.id, c.body, u.display_name, c.created_at
        FROM comments c
        JOIN users u ON u.id = c.user_id
        WHERE c.post_id = $1 AND c.is_hidden = false AND c.is_official_response = true
        ORDER BY c.created_at DESC
        LIMIT 1
        "#,
        post_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| OfficialResponseSummaryDto {
        id: row.id,
        body: row.body,
        display_name: row.display_name,
        created_at: row.created_at,
    }))
}

pub async fn get_status_history(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Vec<crate::dto::StatusHistoryItemDto>, sqlx::Error> {
    sqlx::query_as!(
        crate::dto::StatusHistoryItemDto,
        r#"
        SELECT h.id, h.old_status, h.new_status, h.reason, h.created_at, u.display_name as actor_display_name
        FROM post_status_history h
        JOIN users u ON h.user_id = u.id
        WHERE h.post_id = $1
        ORDER BY h.created_at DESC
        "#,
        post_id
    )
    .fetch_all(pool)
    .await
}

pub async fn get_post_status_context(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Option<PostStatusContextRecord>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT tenant_id, status, title FROM posts WHERE id = $1",
        post_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| PostStatusContextRecord {
        tenant_id: row.tenant_id,
        status: row.status,
        title: row.title,
    }))
}

pub async fn get_post_tenant(pool: &DbPool, post_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query!("SELECT tenant_id FROM posts WHERE id = $1", post_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.tenant_id))
}

pub struct PostStateRecord {
    pub is_hidden: bool,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub async fn get_post_state(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Option<PostStateRecord>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT is_hidden, deleted_at FROM posts WHERE id = $1",
        post_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| PostStateRecord {
        is_hidden: row.is_hidden,
        deleted_at: row.deleted_at,
    }))
}

pub async fn get_post_lock(pool: &DbPool, post_id: Uuid) -> Result<Option<bool>, sqlx::Error> {
    let row = sqlx::query!("SELECT is_locked FROM posts WHERE id = $1", post_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.is_locked))
}

pub struct PostDuplicateContextRecord {
    pub tenant_id: Uuid,
    pub duplicate_of_post_id: Option<Uuid>,
}

pub async fn get_post_duplicate_context(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Option<PostDuplicateContextRecord>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT tenant_id, duplicate_of_post_id FROM posts WHERE id = $1",
        post_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| PostDuplicateContextRecord {
        tenant_id: row.tenant_id,
        duplicate_of_post_id: row.duplicate_of_post_id,
    }))
}

pub struct CanonicalPostStateRecord {
    pub tenant_id: Uuid,
    pub is_hidden: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub duplicate_of_post_id: Option<Uuid>,
}

pub async fn get_canonical_post_state(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Option<CanonicalPostStateRecord>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT tenant_id, is_hidden, deleted_at, duplicate_of_post_id FROM posts WHERE id = $1",
        post_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| CanonicalPostStateRecord {
        tenant_id: row.tenant_id,
        is_hidden: row.is_hidden,
        deleted_at: row.deleted_at,
        duplicate_of_post_id: row.duplicate_of_post_id,
    }))
}

pub async fn update_post_status_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE posts SET status = $1, updated_at = NOW() WHERE id = $2",
        status,
        post_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn insert_status_history_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
    user_id: Uuid,
    old_status: &str,
    new_status: &str,
    reason: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO post_status_history (post_id, user_id, old_status, new_status, reason) VALUES ($1, $2, $3, $4, $5)",
        post_id,
        user_id,
        old_status,
        new_status,
        reason
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn update_post_visibility_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
    is_hidden: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE posts SET is_hidden = $1, updated_at = NOW() WHERE id = $2",
        is_hidden,
        post_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn soft_delete_post_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE posts SET is_hidden = TRUE, deleted_at = NOW(), updated_at = NOW() WHERE id = $1",
        post_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn restore_post_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE posts SET is_hidden = FALSE, deleted_at = NULL, updated_at = NOW() WHERE id = $1",
        post_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn update_post_lock_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
    is_locked: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE posts SET is_locked = $1, updated_at = NOW() WHERE id = $2",
        is_locked,
        post_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn update_duplicate_post_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
    duplicate_of_post_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE posts SET duplicate_of_post_id = $1, updated_at = NOW() WHERE id = $2",
        duplicate_of_post_id,
        post_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn create_post(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    board_id: Uuid,
    user_id: Uuid,
    title: &str,
    body: &str,
    attachments: &[String],
    category_id: Option<Uuid>,
    review_state: &str,
    review_reason: Option<&str>,
) -> Result<crate::dto::PostCreatedDto, sqlx::Error> {
    let attachments_json =
        serde_json::to_value(attachments).unwrap_or_else(|_| serde_json::json!([]));
    let row = sqlx::query(
        r#"
        INSERT INTO posts (tenant_id, board_id, user_id, title, body, attachments, category_id, is_hidden, review_state, review_reason)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id, review_state
        "#,
    )
    .bind(tenant_id)
    .bind(board_id)
    .bind(user_id)
    .bind(title)
    .bind(body)
    .bind(attachments_json)
    .bind(category_id)
    .bind(review_state == "pending")
    .bind(review_state)
    .bind(review_reason)
    .fetch_one(&mut **tx)
    .await?;
    Ok(crate::dto::PostCreatedDto {
        id: row.get("id"),
        review_state: row.get("review_state"),
    })
}

pub async fn get_editable_post(
    pool: &DbPool,
    post_id: Uuid,
) -> Result<Option<EditablePostRecord>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT tenant_id, user_id, is_hidden, deleted_at FROM posts WHERE id = $1",
        post_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| EditablePostRecord {
        tenant_id: row.tenant_id,
        user_id: row.user_id,
        is_hidden: row.is_hidden,
        deleted_at: row.deleted_at,
    }))
}

pub async fn update_post_content(
    pool: &DbPool,
    post_id: Uuid,
    title: &str,
    body: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE posts SET title = $1, body = $2, updated_at = NOW() WHERE id = $3",
        title,
        body,
        post_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_moderation_queue(
    pool: &DbPool,
    tenant_id: Uuid,
    board_slug: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let mut builder = QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT count(*)::bigint
        FROM posts p
        JOIN boards b ON p.board_id = b.id
        WHERE p.tenant_id =
        "#,
    );
    builder.push_bind(tenant_id);
    builder.push(" AND p.deleted_at IS NULL AND (p.review_state = 'pending' OR (p.is_hidden = TRUE AND p.review_state = 'approved')) ");

    if let Some(slug) = board_slug {
        builder.push(" AND b.slug = ");
        builder.push_bind(slug.trim());
    }

    let row: (i64,) = builder.build_query_as().fetch_one(pool).await?;
    Ok(row.0)
}

pub async fn list_moderation_queue(
    pool: &DbPool,
    tenant_id: Uuid,
    board_slug: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<crate::dto::ModerationQueueItemDto>, sqlx::Error> {
    let mut builder = QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT p.id, p.title, p.body, b.slug AS board_slug, p.status, p.is_hidden, p.review_state, p.review_reason, p.deleted_at,
               p.vote_count,
               (SELECT count(*) FROM comments c WHERE c.post_id = p.id AND c.is_hidden = false) AS comment_count,
               p.duplicate_of_post_id, p.created_at, u.display_name AS author_display_name
        FROM posts p
        JOIN boards b ON p.board_id = b.id
        JOIN users u ON p.user_id = u.id
        WHERE p.tenant_id =
        "#,
    );
    builder.push_bind(tenant_id);
    builder.push(" AND p.deleted_at IS NULL AND (p.review_state = 'pending' OR (p.is_hidden = TRUE AND p.review_state = 'approved')) ");

    if let Some(slug) = board_slug {
        builder.push(" AND b.slug = ");
        builder.push_bind(slug.trim());
    }

    builder.push(" ORDER BY p.created_at DESC LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    builder
        .build_query_as::<crate::dto::ModerationQueueItemDto>()
        .fetch_all(pool)
        .await
}

pub async fn get_post_follower_count(pool: &DbPool, post_id: Uuid) -> Result<i64, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT count(*)::bigint AS count FROM post_follows WHERE post_id = $1",
        post_id
    )
    .fetch_one(pool)
    .await?;
    Ok(row.count.unwrap_or(0))
}
