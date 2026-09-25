use crate::{db::DbPool, errors::AppError, users::User};
use uuid::Uuid;

/// A provider's stable subject is the only key used for account lookup.
/// Email is profile data, never an account-linking key.
#[derive(Debug, Clone)]
pub struct ExternalIdentity {
    pub provider_id: String,
    pub subject: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

pub async fn resolve_user(pool: &DbPool, identity: &ExternalIdentity) -> Result<User, AppError> {
    if identity.provider_id.is_empty() || identity.subject.is_empty() {
        return Err(AppError::Unauthorized);
    }
    if identity.provider_id == "rooiam"
        && (identity.subject == "local-admin"
            || identity.subject.starts_with("sso:")
            || identity.subject.starts_with("invited:"))
    {
        return Err(AppError::Unauthorized);
    }
    if let Some(user) = get_by_identity(pool, &identity.provider_id, &identity.subject).await? {
        return Ok(user);
    }

    // Rolling upgrade: old RooIAM rows may have been inserted after the data
    // migration by an older server. Attach their identity without changing IDs.
    if identity.provider_id == "rooiam" {
        let legacy = sqlx::query_as::<_, User>("SELECT id, rooiam_subject, email, display_name, avatar_url, created_at, updated_at FROM users WHERE rooiam_subject = $1")
            .bind(&identity.subject).fetch_optional(pool).await.map_err(db_error)?;
        if let Some(user) = legacy {
            sqlx::query("INSERT INTO user_identities (user_id, provider_id, subject, email) VALUES ($1, 'rooiam', $2, $3) ON CONFLICT (provider_id, subject) DO NOTHING")
                .bind(user.id).bind(&identity.subject).bind(&identity.email).execute(pool).await.map_err(db_error)?;
            return Ok(user);
        }
    }

    let mut tx = pool.begin().await.map_err(db_error)?;
    let user_id = Uuid::new_v4();
    let email = identity.email.as_deref().unwrap_or("");
    let name = identity.name.as_deref().unwrap_or("User");
    let inserted_user = sqlx::query(
        "INSERT INTO users (id, rooiam_subject, email, display_name) VALUES ($1, $2, $3, $4) ON CONFLICT (rooiam_subject) DO NOTHING",
    )
    .bind(user_id)
    .bind(if identity.provider_id == "rooiam" {
        Some(identity.subject.as_str())
    } else {
        None
    })
    .bind(email)
    .bind(name)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    if inserted_user.rows_affected() == 0 {
        tx.rollback().await.map_err(db_error)?;
        let user = sqlx::query_as::<_, User>("SELECT id, rooiam_subject, email, display_name, avatar_url, created_at, updated_at FROM users WHERE rooiam_subject = $1")
            .bind(&identity.subject).fetch_one(pool).await.map_err(db_error)?;
        sqlx::query("INSERT INTO user_identities (user_id, provider_id, subject, email) VALUES ($1, 'rooiam', $2, $3) ON CONFLICT (provider_id, subject) DO NOTHING")
            .bind(user.id).bind(&identity.subject).bind(&identity.email).execute(pool).await.map_err(db_error)?;
        return Ok(user);
    }
    let inserted = sqlx::query(
        "INSERT INTO user_identities (user_id, provider_id, subject, email) VALUES ($1, $2, $3, $4) ON CONFLICT (provider_id, subject) DO NOTHING"
    )
        .bind(user_id).bind(&identity.provider_id).bind(&identity.subject).bind(&identity.email)
        .execute(&mut *tx).await.map_err(db_error)?;
    if inserted.rows_affected() == 0 {
        tx.rollback().await.map_err(db_error)?;
        return get_by_identity(pool, &identity.provider_id, &identity.subject)
            .await?
            .ok_or(AppError::InternalServerError);
    }
    tx.commit().await.map_err(db_error)?;
    get_by_identity(pool, &identity.provider_id, &identity.subject)
        .await?
        .ok_or(AppError::InternalServerError)
}

pub async fn get_by_identity(
    pool: &DbPool,
    provider_id: &str,
    subject: &str,
) -> Result<Option<User>, AppError> {
    sqlx::query_as::<_, User>(
        "SELECT u.id, u.rooiam_subject, u.email, u.display_name, u.avatar_url, u.created_at, u.updated_at FROM user_identities i JOIN users u ON u.id = i.user_id WHERE i.provider_id = $1 AND i.subject = $2"
    )
        .bind(provider_id).bind(subject).fetch_optional(pool).await.map_err(db_error)
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(error = %error, "identity storage error");
    AppError::InternalServerError
}

#[cfg(test)]
mod tests {
    use super::{resolve_user, ExternalIdentity};
    use crate::{
        db,
        http::test_support::{lock_test_db, reset_db, test_settings},
    };

    #[actix_web::test]
    async fn same_email_never_links_different_provider_subjects() {
        let _guard = lock_test_db().await;
        let settings = test_settings();
        let pool = db::establish_connection(&settings.database_url)
            .await
            .unwrap();
        reset_db(&pool).await;
        let one = ExternalIdentity {
            provider_id: "company".into(),
            subject: "one".into(),
            email: Some("same@example.com".into()),
            name: Some("One".into()),
        };
        let two = ExternalIdentity {
            provider_id: "company".into(),
            subject: "two".into(),
            email: Some("same@example.com".into()),
            name: Some("Two".into()),
        };
        let other_provider = ExternalIdentity {
            provider_id: "backup".into(),
            subject: "one".into(),
            email: Some("same@example.com".into()),
            name: Some("One".into()),
        };
        let first = resolve_user(&pool, &one).await.unwrap();
        assert_eq!(resolve_user(&pool, &one).await.unwrap().id, first.id);
        assert_ne!(resolve_user(&pool, &two).await.unwrap().id, first.id);
        assert_ne!(
            resolve_user(&pool, &other_provider).await.unwrap().id,
            first.id
        );
    }
}
