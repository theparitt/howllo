use crate::db::DbPool;
use crate::dto::MembershipItemDto;
use crate::errors::AppError;
use uuid::Uuid;

pub async fn resolve_tenant_id(pool: &DbPool, tenant_slug: &str) -> Result<Uuid, AppError> {
    get_tenant_id_by_slug(pool, tenant_slug)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tenant_slug = tenant_slug, "error resolving tenant");
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound)
}

pub async fn get_tenant_id_by_slug(
    pool: &DbPool,
    tenant_slug: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query!("SELECT id FROM tenants WHERE slug = $1", tenant_slug)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.id))
}

pub async fn list_members(
    pool: &DbPool,
    tenant_id: Uuid,
) -> Result<Vec<MembershipItemDto>, sqlx::Error> {
    sqlx::query_as!(
        MembershipItemDto,
        r#"
        SELECT
            m.user_id,
            u.email,
            u.display_name,
            m.role
        FROM memberships m
        JOIN users u ON u.id = m.user_id
        WHERE m.tenant_id = $1 AND m.public_participant = FALSE
        ORDER BY u.display_name ASC, u.email ASC
        "#,
        tenant_id
    )
    .fetch_all(pool)
    .await
}

pub async fn get_membership<'e, E>(
    executor: E,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Option<MembershipItemDto>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query_as!(
        MembershipItemDto,
        r#"
        SELECT
            m.user_id,
            u.email,
            u.display_name,
            m.role
        FROM memberships m
        JOIN users u ON u.id = m.user_id
        WHERE m.tenant_id = $1 AND m.user_id = $2
        "#,
        tenant_id,
        user_id
    )
    .fetch_optional(executor)
    .await
}

pub struct UserRecord {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
}

pub async fn upsert_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rooiam_subject: &str,
    email: &str,
    display_name: &str,
) -> Result<UserRecord, sqlx::Error> {
    sqlx::query_as!(
        UserRecord,
        r#"
        INSERT INTO users (rooiam_subject, email, display_name)
        VALUES ($1, $2, $3)
        ON CONFLICT (rooiam_subject)
        DO UPDATE SET email = EXCLUDED.email, display_name = EXCLUDED.display_name, updated_at = NOW()
        RETURNING id, email, display_name
        "#,
        rooiam_subject,
        email,
        display_name
    )
    .fetch_one(&mut **tx)
    .await
}

pub async fn upsert_user_on_pool(
    pool: &DbPool,
    rooiam_subject: &str,
    email: &str,
    display_name: &str,
) -> Result<UserRecord, sqlx::Error> {
    sqlx::query_as!(
        UserRecord,
        r#"
        INSERT INTO users (rooiam_subject, email, display_name)
        VALUES ($1, $2, $3)
        ON CONFLICT (rooiam_subject)
        DO UPDATE SET email = EXCLUDED.email, display_name = EXCLUDED.display_name, updated_at = NOW()
        RETURNING id, email, display_name
        "#,
        rooiam_subject,
        email,
        display_name
    )
    .fetch_one(pool)
    .await
}

pub struct MembershipRoleRecord {
    pub role: String,
}

pub async fn get_membership_role(
    pool: &DbPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Option<MembershipRoleRecord>, sqlx::Error> {
    sqlx::query_as!(
        MembershipRoleRecord,
        "SELECT role FROM memberships WHERE tenant_id = $1 AND user_id = $2",
        tenant_id,
        user_id
    )
    .fetch_optional(pool)
    .await
}

pub async fn upsert_membership(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO memberships (tenant_id, user_id, role)
        VALUES ($1, $2, $3)
        ON CONFLICT (tenant_id, user_id)
        DO UPDATE SET role = EXCLUDED.role, public_participant = FALSE
        "#,
        tenant_id,
        user_id,
        role
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn update_membership_role(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE memberships SET role = $1, public_participant = FALSE WHERE tenant_id = $2 AND user_id = $3",
        role,
        tenant_id,
        user_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn delete_membership(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM memberships WHERE tenant_id = $1 AND user_id = $2",
        tenant_id,
        user_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
