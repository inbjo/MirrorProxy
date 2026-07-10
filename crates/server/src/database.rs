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
}
