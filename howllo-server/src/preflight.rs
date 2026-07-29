use std::collections::HashSet;
use std::io;

use reqwest::StatusCode;
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::Executor;

use crate::config::Settings;
use crate::db::{self, DbPool};
use crate::tenancy;

static MIGRATOR: Migrator = sqlx::migrate!("./db/migrations");
const LEGACY_RECONCILABLE_MIGRATIONS: &[i64] = &[
    20240101000000,
    20260604093000,
    20260604203000,
    20260605113000,
];
const MIGRATIONS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
    success BOOLEAN NOT NULL,
    checksum BYTEA NOT NULL,
    execution_time BIGINT NOT NULL
)
"#;

pub async fn run(settings: &Settings) -> io::Result<DbPool> {
    banner("starting up");

    line(
        Level::Ok,
        "server",
        "listen",
        &format!("will bind {}", paint("1", &settings.bind_address)),
    );

    // Each fallible step prints a red failure line + banner before bubbling the
    // error up to main(), which aborts startup. Nothing binds on failure.
    let pool = match check_postgres(settings).await {
        Ok(pool) => pool,
        Err(error) => return Err(fail("postgres", &error)),
    };

    let bootstrap = match tenancy::ensure_bootstrap_tenant(&pool).await {
        Ok(b) => b,
        Err(error) => {
            let io_err = io::Error::new(
                io::ErrorKind::Other,
                format!("bootstrap tenant init failed: {error}"),
            );
            return Err(fail("tenancy", &io_err));
        }
    };
    line(
        Level::Ok,
        "tenancy",
        "default",
        &format!(
            "{} ({})",
            bootstrap.default_tenant_name,
            paint("90", &bootstrap.default_tenant_slug)
        ),
    );

    if let Err(error) = check_storage(&pool).await {
        return Err(fail("storage", &error));
    }

    if let Err(error) = check_ai(settings).await {
        return Err(fail("ai", &error));
    }

    eprintln!();
    line(
        Level::Ok,
        "howllo",
        "ready",
        &paint(
            "1;32",
            &format!("listening on http://{}", settings.bind_address),
        ),
    );
    eprintln!();
    Ok(pool)
}

/// Print a prominent failure block for `system`, then return the error so main
/// can abort. Makes a misconfigured dependency obvious in the logs.
fn fail(system: &str, error: &io::Error) -> io::Error {
    line(
        Level::Fail,
        system,
        "FAILED",
        &paint("1;31", &error.to_string()),
    );
    eprintln!();
    eprintln!(
        "  {}  {}",
        paint("1;31", "[FAIL]"),
        paint("1;31", "startup aborted — fix the issue above and restart")
    );
    eprintln!();
    io::Error::new(error.kind(), error.to_string())
}

async fn check_postgres(settings: &Settings) -> io::Result<DbPool> {
    line(
        Level::Pending,
        "postgres",
        "connect",
        &format!(
            "connecting {}",
            paint("90", &sanitize_database_url(&settings.database_url))
        ),
    );

    let pool = db::establish_connection(&settings.database_url)
        .await
        .map_err(map_database_error)?;

    line(Level::Ok, "postgres", "connect", "connection established");

    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("postgres health check failed: {error}"),
            )
        })?;

    line(Level::Ok, "postgres", "query", "health check passed");

    pool.execute(MIGRATIONS_TABLE_SQL).await.map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("postgres migration table init failed: {error}"),
        )
    })?;

    let applied_versions: Vec<i64> = sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations WHERE success = true ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("postgres migration state check failed: {error}"),
        )
    })?;

    let applied_set: HashSet<i64> = applied_versions.iter().copied().collect();
    let pending_migrations = MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .filter(|migration| !applied_set.contains(&migration.version))
        .count();

    let applied_count = applied_versions.len();
    if pending_migrations == 0 {
        line(
            Level::Ok,
            "postgres",
            "migrations",
            &format!("{applied_count} applied, 0 pending"),
        );
    } else {
        line(
            Level::Pending,
            "postgres",
            "migrations",
            &format!("{applied_count} applied, applying {pending_migrations} pending…"),
        );
    }

    match MIGRATOR.run(&pool).await {
        Ok(()) => {}
        Err(MigrateError::VersionMismatch(version))
            if LEGACY_RECONCILABLE_MIGRATIONS.contains(&version) =>
        {
            line(
                Level::Pending,
                "postgres",
                "migrations",
                &format!("checksum mismatch on {version}, reconciling…"),
            );
            reconcile_migration_checksum(&pool, version).await?;
            if pending_migrations > 0 {
                MIGRATOR.run(&pool).await.map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        format!("postgres migration failed after checksum repair: {error}"),
                    )
                })?;
            }
        }
        Err(error) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("postgres migration failed: {error}"),
            ));
        }
    }

    line(Level::Ok, "postgres", "migrations", "up to date");
    Ok(pool)
}

async fn check_ai(settings: &Settings) -> io::Result<()> {
    if !settings.ai.enabled {
        line(Level::Skip, "ai", "endpoint", "disabled");
        return Ok(());
    }

    line(
        Level::Pending,
        "ai",
        "endpoint",
        &format!("checking {} ({})", settings.ai.base_url, settings.ai.model),
    );

    let response = reqwest::get(&settings.ai.base_url).await.map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("ai endpoint check failed: {error}"),
        )
    })?;

    let response_status = response.status();
    if !response_status.is_success() && response_status != StatusCode::NOT_FOUND {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("ai endpoint returned unexpected status: {response_status}"),
        ));
    }

    line(
        Level::Ok,
        "ai",
        "endpoint",
        &format!("reachable ({response_status})"),
    );
    Ok(())
}

/// Verify the effective storage backend works at startup, so a misconfigured
/// MinIO (or unwritable local dir) breaks the boot rather than failing later on
/// the first upload. Reads the effective config (env defaults + any DB override).
async fn check_storage(pool: &DbPool) -> io::Result<()> {
    use crate::storage::{
        load_platform_storage_config, test_local_storage, test_minio_storage, StorageBackend,
    };

    let cfg = load_platform_storage_config(pool).await.map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("could not load storage config: {error}"),
        )
    })?;

    match cfg.backend {
        StorageBackend::Local => {
            line(
                Level::Pending,
                "storage",
                "local",
                &format!("checking {}", paint("90", &cfg.local_path)),
            );
            test_local_storage(&cfg.local_path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            line(Level::Ok, "storage", "local", "writable");
        }
        StorageBackend::Minio => {
            // The effective secret comes from DB-override or HOWLLO_MINIO_PASSWORD;
            // load_platform_storage_config only reports whether it's configured, so
            // re-read it the same way the test endpoint does.
            let secret = minio_secret(pool).await;
            line(
                Level::Pending,
                "storage",
                "minio",
                &format!(
                    "checking {} (bucket {})",
                    paint("90", &cfg.minio_endpoint),
                    paint("90", &cfg.minio_bucket)
                ),
            );
            test_minio_storage(
                &cfg.minio_endpoint,
                &cfg.minio_bucket,
                &cfg.minio_access_key,
                &secret,
                cfg.minio_use_ssl,
            )
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            line(
                Level::Ok,
                "storage",
                "minio",
                &format!("bucket {} reachable & public-read", cfg.minio_bucket),
            );
        }
    }
    Ok(())
}

/// Effective MinIO secret for the startup check: DB override, else env password.
async fn minio_secret(pool: &DbPool) -> String {
    let stored = sqlx::query_scalar::<_, String>(
        "SELECT value FROM system_settings WHERE key = 'storage_minio_secret_key'",
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
    .unwrap_or_default();
    if !stored.trim().is_empty() {
        return stored.trim().to_string();
    }
    std::env::var("HOWLLO_MINIO_PASSWORD")
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn map_database_error(error: sqlx::Error) -> io::Error {
    match error {
        sqlx::Error::Database(db_error) => {
            let code = db_error
                .code()
                .map(|value| value.to_string())
                .unwrap_or_default();
            let message = match code.as_str() {
                "28P01" => format!(
                    "postgres authentication failed. Check HOWLLO_DATABASE_URL username/password. Original error: {}",
                    db_error.message()
                ),
                "3D000" => format!(
                    "postgres database does not exist. Create it first or fix HOWLLO_DATABASE_URL. Original error: {}",
                    db_error.message()
                ),
                _ => format!("postgres connection failed: {}", db_error.message()),
            };
            io::Error::new(io::ErrorKind::Other, message)
        }
        other => io::Error::new(
            io::ErrorKind::Other,
            format!("postgres connection failed: {other}"),
        ),
    }
}

fn sanitize_database_url(database_url: &str) -> String {
    match url::Url::parse(database_url) {
        Ok(mut parsed) => {
            if parsed.password().is_some() {
                let _ = parsed.set_password(Some("******"));
            }
            parsed.to_string()
        }
        Err(_) => database_url.to_string(),
    }
}

// ── Pretty startup output ──────────────────────────────────────────────────────
// A human-readable, colorized summary printed straight to stderr (not through
// the structured tracing logger). Each line leads with a status symbol so the
// operator can see at a glance what is OK, pending, skipped, or failed.

/// Whether to emit ANSI color. Honors NO_COLOR and only colors a real terminal.
fn use_color() -> bool {
    use std::io::IsTerminal;
    std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal()
}

fn paint(code: &str, text: &str) -> String {
    if use_color() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// Status level for a preflight line — drives the symbol and color.
#[derive(Clone, Copy)]
pub enum Level {
    /// A check passed.
    Ok,
    /// Work in progress / connecting.
    Pending,
    /// Intentionally skipped or not part of this build.
    Skip,
    /// Informational (no pass/fail).
    Info,
    /// A check failed.
    Fail,
}

impl Level {
    /// Fixed-width bracketed text tag, e.g. "[ OK ]". Easy to read at a glance.
    fn tag(self) -> &'static str {
        match self {
            Level::Ok => "[ OK ]",
            Level::Pending => "[ .. ]",
            Level::Skip => "[SKIP]",
            Level::Info => "[INFO]",
            Level::Fail => "[FAIL]",
        }
    }

    /// ANSI code: bold colored text (no background) for an easy-to-read tag.
    fn color(self) -> &'static str {
        match self {
            Level::Ok => "1;32",      // green
            Level::Pending => "1;33", // yellow
            Level::Skip => "1;90",    // grey
            Level::Info => "1;36",    // cyan
            Level::Fail => "1;31",    // red
        }
    }
}

fn banner(message: &str) {
    let line = "─".repeat(52);
    eprintln!();
    eprintln!("  {}", paint("38;5;209", &line)); // coral-ish
    eprintln!(
        "  {}  {}",
        paint("38;5;209", "🐺"),
        paint("1;38;5;209", &format!("howllo · {message}"))
    );
    eprintln!("  {}", paint("38;5;209", &line));
}

/// Print one status line: `[ OK ] <system>  <step>  <message>`.
fn line(level: Level, system: &str, step: &str, message: &str) {
    let tag = paint(level.color(), level.tag());
    let system = paint("1", &format!("{system:<9}"));
    let step = paint("90", &format!("{step:<12}"));
    eprintln!("  {tag}  {system} {step} {message}");
}

async fn reconcile_migration_checksum(pool: &DbPool, version: i64) -> io::Result<()> {
    let migration = MIGRATOR
        .iter()
        .find(|migration| migration.version == version)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("migration checksum repair failed: version {version} not found locally"),
            )
        })?;

    sqlx::query("UPDATE _sqlx_migrations SET checksum = $1 WHERE version = $2 AND success = TRUE")
        .bind(migration.checksum.as_ref())
        .bind(version)
        .execute(pool)
        .await
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("migration checksum repair failed for {version}: {error}"),
            )
        })?;

    line(
        Level::Ok,
        "postgres",
        "migrations",
        &format!("checksum repaired for {version}"),
    );
    Ok(())
}
