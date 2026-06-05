use uuid::Uuid;

pub async fn insert_vote(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query!(
        "INSERT INTO post_votes (post_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        post_id,
        user_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(res.rows_affected())
}

pub async fn increment_vote_count(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE posts SET vote_count = vote_count + 1 WHERE id = $1",
        post_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn delete_vote(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query!(
        "DELETE FROM post_votes WHERE post_id = $1 AND user_id = $2",
        post_id,
        user_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(res.rows_affected())
}

pub async fn decrement_vote_count(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE posts SET vote_count = vote_count - 1 WHERE id = $1",
        post_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
