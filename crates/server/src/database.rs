use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::{distributions::Alphanumeric, Rng};
use sha2::{Digest, Sha256};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};

use crate::config::Config;

const SESSION_LIFETIME_SECS: i64 = 24 * 60 * 60;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

pub struct InitialAdminCredentials {
    pub username: &'static str,
    pub password: String,
}

pub struct AdminSession {
    pub token: String,
    pub expires_at: i64,
}

pub struct ProxyTrafficRecord<'a> {
    pub day: &'a str,
    pub month: &'a str,
    pub target_code: &'a str,
    pub method: &'a str,
    pub path: &'a str,
    pub status_code: u16,
    pub response_bytes: u64,
    pub stream_error: bool,
}

impl Database {
    pub async fn open(
        database_path: &str,
    ) -> anyhow::Result<(Self, Option<InitialAdminCredentials>)> {
        if database_path != ":memory:" {
            ensure_parent_directory(database_path)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(database_path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(if database_path == ":memory:" { 1 } else { 5 })
            .connect_with(options)
            .await
            .context("failed to open SQLite database")?;
        let database = Self { pool };
        database.migrate().await?;
        let credentials = database.ensure_initial_admin().await?;
        Ok((database, credentials))
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        for statement in MIGRATIONS {
            sqlx::query(statement)
                .execute(&self.pool)
                .await
                .context("failed to apply SQLite migration")?;
        }
        Ok(())
    }

    async fn ensure_initial_admin(&self) -> anyhow::Result<Option<InitialAdminCredentials>> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM admin_users")
            .fetch_one(&self.pool)
            .await?;
        let count: i64 = row.try_get("count")?;
        if count > 0 {
            return Ok(None);
        }

        let password = random_secret(28);
        let password_hash = hash_password(&password)?;
        let now = unix_timestamp();
        sqlx::query(
            "INSERT INTO admin_users (username, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind("admin")
        .bind(password_hash)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(Some(InitialAdminCredentials {
            username: "admin",
            password,
        }))
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> anyhow::Result<Option<AdminSession>> {
        let row = sqlx::query("SELECT password_hash FROM admin_users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let password_hash: String = row.try_get("password_hash")?;
        if !verify_password(password, &password_hash) {
            return Ok(None);
        }

        let now = unix_timestamp();
        let expires_at = now + SESSION_LIFETIME_SECS;
        let token = random_secret(48);
        sqlx::query(
            "INSERT INTO admin_sessions (token_hash, username, created_at, expires_at, last_used_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(hash_token(&token))
        .bind(username)
        .bind(now)
        .bind(expires_at)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(Some(AdminSession { token, expires_at }))
    }

    pub async fn authorize(&self, token: &str) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        sqlx::query("DELETE FROM admin_sessions WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        let result = sqlx::query(
            "UPDATE admin_sessions SET last_used_at = ? WHERE token_hash = ? AND expires_at > ?",
        )
        .bind(now)
        .bind(hash_token(token))
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn logout(&self, token: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM admin_sessions WHERE token_hash = ?")
            .bind(hash_token(token))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn load_or_seed_runtime_config(&self, fallback: Config) -> anyhow::Result<Config> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = 'runtime_config'")
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            self.save_runtime_config("system", &fallback, "seed runtime configuration")
                .await?;
            return Ok(fallback);
        };
        let value: String = row.try_get("value")?;
        let config: Config =
            serde_json::from_str(&value).context("stored runtime configuration is invalid JSON")?;
        config
            .validate()
            .context("stored runtime configuration is invalid")?;
        Ok(config)
    }

    pub async fn save_runtime_config(
        &self,
        username: &str,
        config: &Config,
        action: &str,
    ) -> anyhow::Result<()> {
        let value = serde_json::to_string(config)?;
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO settings (key, value, version, updated_at) VALUES ('runtime_config', ?, 1, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value, version = settings.version + 1, updated_at = excluded.updated_at",
        )
        .bind(value)
        .bind(now)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, ?, ?)",
        )
        .bind(now)
        .bind(username)
        .bind(action)
        .bind("runtime_config")
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn monthly_response_bytes(&self, month: &str) -> anyhow::Result<u64> {
        let row = sqlx::query("SELECT response_bytes FROM traffic_monthly WHERE month = ?")
            .bind(month)
            .fetch_optional(&self.pool)
            .await?;
        let bytes = row
            .map(|row| row.try_get::<i64, _>("response_bytes"))
            .transpose()?
            .unwrap_or(0);
        Ok(bytes.max(0) as u64)
    }

    pub async fn record_proxy_response(
        &self,
        record: ProxyTrafficRecord<'_>,
    ) -> anyhow::Result<()> {
        let bytes = i64::try_from(record.response_bytes).unwrap_or(i64::MAX);
        let errors = i64::from(record.stream_error || record.status_code >= 400);
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO traffic_daily (day, target_code, request_count, response_bytes, upstream_bytes, error_count) VALUES (?, ?, 1, ?, 0, ?) ON CONFLICT(day, target_code) DO UPDATE SET request_count = request_count + 1, response_bytes = response_bytes + excluded.response_bytes, error_count = error_count + excluded.error_count",
        )
        .bind(record.day)
        .bind(record.target_code)
        .bind(bytes)
        .bind(errors)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO traffic_monthly (month, request_count, response_bytes, upstream_bytes, error_count, quota_exceeded) VALUES (?, 1, ?, 0, ?, 0) ON CONFLICT(month) DO UPDATE SET request_count = request_count + 1, response_bytes = response_bytes + excluded.response_bytes, error_count = error_count + excluded.error_count",
        )
        .bind(record.month)
        .bind(bytes)
        .bind(errors)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO request_events (created_at, target_code, method, path, status_code, response_bytes) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(now)
        .bind(record.target_code)
        .bind(record.method)
        .bind(record.path)
        .bind(i64::from(record.status_code))
        .bind(bytes)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }
}

fn ensure_parent_directory(database_path: &str) -> anyhow::Result<()> {
    let path = Path::new(database_path);
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create database directory {}", parent.display()))?;
    }
    Ok(())
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| anyhow::anyhow!("failed to hash administrator password: {error}"))?
        .to_string())
}

fn verify_password(password: &str, password_hash: &str) -> bool {
    PasswordHash::new(password_hash)
        .ok()
        .and_then(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .ok()
        })
        .is_some()
}

fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn random_secret(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before Unix epoch")
        .as_secs() as i64
}

const MIGRATIONS: &[&str] = &[
    "PRAGMA journal_mode = WAL",
    "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, version INTEGER NOT NULL DEFAULT 1, updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS admin_users (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS admin_sessions (token_hash TEXT PRIMARY KEY, username TEXT NOT NULL, created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, last_used_at INTEGER NOT NULL, FOREIGN KEY(username) REFERENCES admin_users(username) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS proxy_targets (code TEXT PRIMARY KEY, enabled INTEGER NOT NULL, upstream_url TEXT NOT NULL, route_prefix TEXT NOT NULL, updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS traffic_daily (day TEXT NOT NULL, target_code TEXT NOT NULL, request_count INTEGER NOT NULL DEFAULT 0, response_bytes INTEGER NOT NULL DEFAULT 0, upstream_bytes INTEGER NOT NULL DEFAULT 0, error_count INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(day, target_code))",
    "CREATE TABLE IF NOT EXISTS traffic_monthly (month TEXT PRIMARY KEY, request_count INTEGER NOT NULL DEFAULT 0, response_bytes INTEGER NOT NULL DEFAULT 0, upstream_bytes INTEGER NOT NULL DEFAULT 0, error_count INTEGER NOT NULL DEFAULT 0, quota_exceeded INTEGER NOT NULL DEFAULT 0)",
    "CREATE TABLE IF NOT EXISTS request_events (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at INTEGER NOT NULL, target_code TEXT, method TEXT NOT NULL, path TEXT NOT NULL, status_code INTEGER NOT NULL, response_bytes INTEGER NOT NULL DEFAULT 0)",
    "CREATE TABLE IF NOT EXISTS config_audit_log (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at INTEGER NOT NULL, username TEXT NOT NULL, action TEXT NOT NULL, detail TEXT NOT NULL)",
    "CREATE INDEX IF NOT EXISTS admin_sessions_expires_at_idx ON admin_sessions(expires_at)",
    "CREATE INDEX IF NOT EXISTS request_events_created_at_idx ON request_events(created_at)",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initializes_admin_and_manages_sessions() {
        let (database, credentials) = Database::open(":memory:").await.unwrap();
        let credentials = credentials.expect("first database open should create admin");
        assert_eq!(credentials.username, "admin");
        assert!(database
            .login("admin", "wrong-password")
            .await
            .unwrap()
            .is_none());

        let session = database
            .login("admin", &credentials.password)
            .await
            .unwrap()
            .unwrap();
        assert!(database.authorize(&session.token).await.unwrap());
        database.logout(&session.token).await.unwrap();
        assert!(!database.authorize(&session.token).await.unwrap());
    }

    #[tokio::test]
    async fn seeds_and_reloads_runtime_config() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let mut config = Config {
            public_base_url: "https://mirror.example".to_string(),
            ..Config::default()
        };
        let seeded = database
            .load_or_seed_runtime_config(config.clone())
            .await
            .unwrap();
        assert_eq!(seeded.public_base_url, "https://mirror.example");

        config.quota.monthly_gb = 42;
        database
            .save_runtime_config("admin", &config, "update runtime configuration")
            .await
            .unwrap();
        let loaded = database
            .load_or_seed_runtime_config(Config::default())
            .await
            .unwrap();
        assert_eq!(loaded.quota.monthly_gb, 42);
    }

    #[tokio::test]
    async fn records_monthly_and_daily_proxy_traffic() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        database
            .record_proxy_response(ProxyTrafficRecord {
                day: "2026-07-10",
                month: "2026-07",
                target_code: "npm",
                method: "GET",
                path: "/npm/react",
                status_code: 200,
                response_bytes: 1024,
                stream_error: false,
            })
            .await
            .unwrap();
        database
            .record_proxy_response(ProxyTrafficRecord {
                day: "2026-07-10",
                month: "2026-07",
                target_code: "npm",
                method: "GET",
                path: "/npm/missing",
                status_code: 404,
                response_bytes: 12,
                stream_error: false,
            })
            .await
            .unwrap();

        assert_eq!(
            database.monthly_response_bytes("2026-07").await.unwrap(),
            1036
        );
    }
}
