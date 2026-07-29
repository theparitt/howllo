use uuid::Uuid;

/// Find the account this user already owns, or create a fresh one for them.
/// Runs inside the caller's transaction so workspace creation stays atomic.
/// All queries are untyped so no sqlx offline cache regeneration is needed.
pub async fn find_or_create_account_for_owner(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner_user_id: Uuid,
    fallback_name: &str,
) -> Result<Uuid, sqlx::Error> {
    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT account_id
        FROM account_memberships
        WHERE user_id = $1 AND role = 'owner'
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .bind(owner_user_id)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(id);
    }

    let name = {
        let trimmed = fallback_name.trim();
        if trimmed.is_empty() {
            "Workspace"
        } else {
            trimmed
        }
    };
    let slug = format!(
        "{}-{}",
        slugify(name),
        &Uuid::new_v4().simple().to_string()[..6]
    );

    let account_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO accounts (slug, name, owner_user_id)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
    )
    .bind(&slug)
    .bind(name)
    .bind(owner_user_id)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO account_memberships (account_id, user_id, role)
        VALUES ($1, $2, 'owner')
        ON CONFLICT (account_id, user_id) DO NOTHING
        "#,
    )
    .bind(account_id)
    .bind(owner_user_id)
    .execute(&mut **tx)
    .await?;

    Ok(account_id)
}

fn slugify(value: &str) -> String {
    let lowered: String = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let collapsed = lowered
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "acct".to_string()
    } else {
        collapsed
    }
}
