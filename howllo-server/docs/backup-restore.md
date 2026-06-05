# Backup and Restore

## Backup

```bash
pg_dump -U postgres howllo > howllo_backup_$(date +%Y%m%d).sql
```

For compressed backup:
```bash
pg_dump -U postgres howllo | gzip > howllo_backup_$(date +%Y%m%d).sql.gz
```

## Restore

```bash
psql -U postgres -c "CREATE DATABASE howllo_restored;"
psql -U postgres howllo_restored < howllo_backup_20260603.sql
```

Then run migrations to ensure schema is current:
```bash
DATABASE_URL=postgres://postgres:password@localhost:5432/howllo_restored sqlx migrate run --source db/migrations
```

## Automated Backups

Example cron job:
```bash
0 2 * * * /usr/bin/pg_dump -U postgres howllo | gzip > /backups/howllo_$(date +\%Y\%m\%d).sql.gz
```

## Key Tables to Backup

All tables are backed up by `pg_dump`:
- `tenants`, `boards`, `posts`, `comments`
- `post_votes`, `post_follows`, `post_tags`, `tags`
- `post_status_history`, `memberships`
- `notifications`, `moderation_notes`
- `audit_logs`, `webhook_endpoints`, `webhook_events`, `webhook_deliveries`
- `api_tokens`, `ai_suggestions`

## Migration Safety

All migrations use `IF NOT EXISTS` and `ADD COLUMN IF NOT EXISTS`. They are replayable and idempotent. You can safely run migrations on an existing database.
