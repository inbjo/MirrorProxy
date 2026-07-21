use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::{distributions::Alphanumeric, rngs::OsRng as RandomOsRng, Rng, RngCore};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqids::Sqids;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow},
    Row, Sqlite, SqlitePool, Transaction,
};
use uuid::Uuid;
use webauthn_rs::prelude::{AuthenticationResult, Passkey};

use crate::config::Config;

const SESSION_LIFETIME_SECS: i64 = 24 * 60 * 60;
const ADMIN_LOGIN_FAILURE_LIMIT: i64 = 5;
const ADMIN_LOGIN_LOCK_SECS: i64 = 15 * 60;
const WEBAUTHN_CHALLENGE_LIFETIME_SECS: i64 = 5 * 60;
const RECENT_ADMIN_VERIFICATION_SECS: i64 = 10 * 60;
const ROUTING_ID_INSERT_ATTEMPTS: usize = 16;
const USER_SESSION_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

pub struct InitialAdminCredentials {
    pub username: &'static str,
    pub password: String,
    pub generated: bool,
}

#[derive(Debug)]
pub struct AdminSession {
    pub token: String,
    pub expires_at: i64,
    pub username: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct AdminSessionSummary {
    pub id: String,
    pub auth_method: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_used_at: i64,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AdminIdentity {
    pub username: String,
    pub role: String,
}

#[derive(Debug)]
pub enum AdminLoginOutcome {
    Success(AdminSession),
    Invalid,
    Locked { retry_after_secs: u64 },
}

#[derive(Debug, Serialize)]
pub struct AdminAccount {
    pub username: String,
    pub role: String,
    pub disabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminPasskeySummary {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

pub struct StoredAdminPasskey {
    pub id: i64,
    pub passkey: Passkey,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct UserAccount {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    pub disabled: bool,
    pub routing_id: String,
    pub routing_rotated_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct UserIdentity {
    pub user_id: i64,
    pub email: String,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserRoutingIdentity {
    pub user_id: i64,
    pub routing_id: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RoutingRotationOutcome {
    Rotated { routing_id: String },
    Cooldown { retry_after_secs: u64 },
    NotFound,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SmtpSettings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub encrypted_password: Option<String>,
    pub from_name: String,
    pub from_address: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmailInvitation {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    pub status: String,
    pub expires_at: i64,
    pub created_at: i64,
}

#[derive(Clone, Debug)]
pub struct OutboxMessage {
    pub id: i64,
    pub recipient: String,
    pub subject: String,
    pub encrypted_body: String,
    pub attempts: u32,
}

#[derive(Clone, Debug)]
pub struct UserSession {
    pub token: String,
    pub expires_at: i64,
    pub identity: UserIdentity,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AuthProvider {
    pub id: i64,
    pub slug: String,
    pub display_name: String,
    pub kind: String,
    pub preset: String,
    pub enabled: bool,
    pub client_id: String,
    #[serde(skip_serializing)]
    pub encrypted_client_secret: Option<String>,
    pub issuer_url: Option<String>,
    pub authorization_url: Option<String>,
    pub token_url: Option<String>,
    pub userinfo_url: Option<String>,
    pub emails_url: Option<String>,
    pub scopes: Vec<String>,
    pub subject_field: String,
    pub email_field: String,
    pub email_verified_field: Option<String>,
    pub display_name_field: String,
    pub allow_registration: bool,
    pub auto_link_by_email: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ExternalUserIdentity {
    pub id: i64,
    pub provider_slug: String,
    pub provider_name: String,
    pub provider_subject: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug)]
pub struct StoredAuthFlow {
    pub provider_id: i64,
    pub encrypted_payload: String,
    pub mode: String,
    pub user_id: Option<i64>,
}

pub struct ExternalRegistration<'a> {
    pub actor: &'a str,
    pub email: &'a str,
    pub display_name: &'a str,
    pub routing_min_length: u8,
    pub provider_slug: &'a str,
    pub provider_subject: &'a str,
    pub invitation_id: Option<i64>,
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
    pub reserved_bytes: u64,
    pub user_id: Option<i64>,
    pub group_id: Option<i64>,
    pub request_event_retention_days: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BillingGroup {
    pub id: i64,
    pub name: String,
    pub monthly_limit_bytes: Option<u64>,
    pub member_count: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct UserBillingProfile {
    pub user_id: i64,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
    pub group_monthly_limit_bytes: Option<u64>,
    pub quota_mode: String,
    pub user_monthly_limit_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HierarchicalReservationOutcome {
    Reserved { group_id: Option<i64> },
    Exceeded { scope: &'static str },
}

#[derive(Serialize)]
pub struct TrafficDailyPoint {
    pub day: String,
    pub target_code: String,
    pub request_count: u64,
    pub response_bytes: u64,
    pub error_count: u64,
}

#[derive(Serialize)]
pub struct TrafficTargetPoint {
    pub target_code: String,
    pub request_count: u64,
    pub response_bytes: u64,
    pub error_count: u64,
}

#[derive(Serialize)]
pub struct TrafficOverview {
    pub request_count: u64,
    pub response_bytes: u64,
    pub error_count: u64,
    pub quota_exceeded: bool,
    pub daily: Vec<TrafficDailyPoint>,
    pub targets: Vec<TrafficTargetPoint>,
}

#[derive(Serialize)]
pub struct QuotaUsage {
    pub limit_bytes: Option<u64>,
    pub used_bytes: u64,
    pub remaining_bytes: Option<u64>,
}

#[derive(Serialize)]
pub struct GroupUsage {
    pub id: i64,
    pub name: String,
    pub quota: QuotaUsage,
}

#[derive(Serialize)]
pub struct UserUsageOverview {
    pub month: String,
    pub today_response_bytes: u64,
    pub request_count: u64,
    pub response_bytes: u64,
    pub error_count: u64,
    pub quota: QuotaUsage,
    pub group: Option<GroupUsage>,
    pub daily: Vec<TrafficDailyPoint>,
    pub targets: Vec<TrafficTargetPoint>,
}

#[derive(Serialize)]
pub struct AuditLogEntry {
    pub created_at: i64,
    pub username: String,
    pub action: String,
    pub detail: String,
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
        let has_reservation_column = sqlx::query("PRAGMA table_info(traffic_monthly)")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .any(|row| row.try_get::<String, _>("name").ok().as_deref() == Some("reserved_bytes"));
        if !has_reservation_column {
            sqlx::query(
                "ALTER TABLE traffic_monthly ADD COLUMN reserved_bytes INTEGER NOT NULL DEFAULT 0",
            )
            .execute(&self.pool)
            .await
            .context("failed to add traffic reservation column")?;
        }
        self.ensure_admin_columns().await?;
        Ok(())
    }

    async fn ensure_admin_columns(&self) -> anyhow::Result<()> {
        let rows = sqlx::query("PRAGMA table_info(admin_users)")
            .fetch_all(&self.pool)
            .await?;
        let columns = rows
            .iter()
            .filter_map(|row| row.try_get::<String, _>("name").ok())
            .collect::<Vec<_>>();
        for (column, statement) in [
            (
                "role",
                "ALTER TABLE admin_users ADD COLUMN role TEXT NOT NULL DEFAULT 'super_admin'",
            ),
            (
                "disabled",
                "ALTER TABLE admin_users ADD COLUMN disabled INTEGER NOT NULL DEFAULT 0",
            ),
            (
                "failed_login_count",
                "ALTER TABLE admin_users ADD COLUMN failed_login_count INTEGER NOT NULL DEFAULT 0",
            ),
            (
                "locked_until",
                "ALTER TABLE admin_users ADD COLUMN locked_until INTEGER",
            ),
            (
                "user_handle",
                "ALTER TABLE admin_users ADD COLUMN user_handle TEXT",
            ),
        ] {
            if !columns.iter().any(|existing| existing == column) {
                sqlx::query(statement)
                    .execute(&self.pool)
                    .await
                    .with_context(|| format!("failed to add admin_users.{column}"))?;
            }
        }
        let users = sqlx::query("SELECT username FROM admin_users WHERE user_handle IS NULL")
            .fetch_all(&self.pool)
            .await?;
        for row in users {
            let username: String = row.try_get("username")?;
            sqlx::query("UPDATE admin_users SET user_handle = ? WHERE username = ?")
                .bind(Uuid::new_v4().to_string())
                .bind(username)
                .execute(&self.pool)
                .await?;
        }
        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS admin_users_user_handle_idx ON admin_users(user_handle)",
        )
        .execute(&self.pool)
        .await?;

        let session_rows = sqlx::query("PRAGMA table_info(admin_sessions)")
            .fetch_all(&self.pool)
            .await?;
        let session_columns = session_rows
            .iter()
            .filter_map(|row| row.try_get::<String, _>("name").ok())
            .collect::<Vec<_>>();
        for (column, statement) in [
            (
                "auth_method",
                "ALTER TABLE admin_sessions ADD COLUMN auth_method TEXT NOT NULL DEFAULT 'password'",
            ),
            (
                "verified_at",
                "ALTER TABLE admin_sessions ADD COLUMN verified_at INTEGER NOT NULL DEFAULT 0",
            ),
        ] {
            if !session_columns.iter().any(|existing| existing == column) {
                sqlx::query(statement)
                    .execute(&self.pool)
                    .await
                    .with_context(|| format!("failed to add admin_sessions.{column}"))?;
            }
        }
        sqlx::query("UPDATE admin_sessions SET verified_at = created_at WHERE verified_at = 0")
            .execute(&self.pool)
            .await?;
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

        let (password, generated) =
            initial_admin_password(std::env::var("MIRRORPROXY_ADMIN_PASSWORD").ok());
        let password_hash = hash_password(&password)?;
        let now = unix_timestamp();
        sqlx::query(
            "INSERT INTO admin_users (username, password_hash, user_handle, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("admin")
        .bind(password_hash)
        .bind(Uuid::new_v4().to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(Some(InitialAdminCredentials {
            username: "admin",
            password,
            generated,
        }))
    }

    #[cfg(test)]
    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> anyhow::Result<Option<AdminSession>> {
        match self
            .login_with_context(username, password, "unknown")
            .await?
        {
            AdminLoginOutcome::Success(session) => Ok(Some(session)),
            AdminLoginOutcome::Invalid | AdminLoginOutcome::Locked { .. } => Ok(None),
        }
    }

    pub async fn login_with_context(
        &self,
        username: &str,
        password: &str,
        source: &str,
    ) -> anyhow::Result<AdminLoginOutcome> {
        let username = username.trim();
        let row = sqlx::query(
            "SELECT password_hash, role, disabled, failed_login_count, locked_until FROM admin_users WHERE username = ?",
        )
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            self.write_security_audit(username, "admin_login_failed", source)
                .await?;
            return Ok(AdminLoginOutcome::Invalid);
        };
        let now = unix_timestamp();
        let disabled: bool = row.try_get("disabled")?;
        let locked_until: Option<i64> = row.try_get("locked_until")?;
        if disabled {
            self.write_security_audit(username, "admin_login_failed", source)
                .await?;
            return Ok(AdminLoginOutcome::Invalid);
        }
        if locked_until.is_some_and(|until| until > now) {
            self.write_security_audit(username, "admin_login_locked", source)
                .await?;
            return Ok(AdminLoginOutcome::Locked {
                retry_after_secs: locked_until.unwrap_or(now).saturating_sub(now) as u64,
            });
        }
        let password_hash: String = row.try_get("password_hash")?;
        if !verify_password(password, &password_hash) {
            let previous_failures: i64 = row.try_get("failed_login_count")?;
            let failures = if locked_until.is_some() {
                1
            } else {
                previous_failures + 1
            };
            let next_locked_until =
                (failures >= ADMIN_LOGIN_FAILURE_LIMIT).then_some(now + ADMIN_LOGIN_LOCK_SECS);
            sqlx::query(
                "UPDATE admin_users SET failed_login_count = ?, locked_until = ?, updated_at = ? WHERE username = ?",
            )
            .bind(failures)
            .bind(next_locked_until)
            .bind(now)
            .bind(username)
            .execute(&self.pool)
            .await?;
            self.write_security_audit(username, "admin_login_failed", source)
                .await?;
            return Ok(next_locked_until.map_or(AdminLoginOutcome::Invalid, |_| {
                AdminLoginOutcome::Locked {
                    retry_after_secs: ADMIN_LOGIN_LOCK_SECS as u64,
                }
            }));
        }

        let role: String = row.try_get("role")?;
        let expires_at = now + SESSION_LIFETIME_SECS;
        let token = random_secret(48);
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO admin_sessions (token_hash, username, auth_method, created_at, expires_at, last_used_at, verified_at) VALUES (?, ?, 'password', ?, ?, ?, ?)",
        )
        .bind(hash_token(&token))
        .bind(username)
        .bind(now)
        .bind(expires_at)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE admin_users SET failed_login_count = 0, locked_until = NULL, updated_at = ? WHERE username = ?",
        )
        .bind(now)
        .bind(username)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'admin_login_succeeded', ?)",
        )
        .bind(now)
        .bind(username)
        .bind(source)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(AdminLoginOutcome::Success(AdminSession {
            token,
            expires_at,
            username: username.to_string(),
            role,
        }))
    }

    #[cfg(test)]
    pub async fn authorize(&self, token: &str) -> anyhow::Result<bool> {
        Ok(self.authenticate_session(token).await?.is_some())
    }

    pub async fn authenticate_session(&self, token: &str) -> anyhow::Result<Option<AdminIdentity>> {
        let now = unix_timestamp();
        sqlx::query("DELETE FROM admin_sessions WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        let row = sqlx::query(
            "SELECT s.username, u.role FROM admin_sessions s JOIN admin_users u ON u.username = s.username WHERE s.token_hash = ? AND s.expires_at > ? AND u.disabled = 0",
        )
        .bind(hash_token(token))
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let username: String = row.try_get("username")?;
        let role: String = row.try_get("role")?;
        sqlx::query("UPDATE admin_sessions SET last_used_at = ? WHERE token_hash = ?")
            .bind(now)
            .bind(hash_token(token))
            .execute(&self.pool)
            .await?;
        Ok(Some(AdminIdentity { username, role }))
    }

    pub async fn logout(&self, token: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM admin_sessions WHERE token_hash = ?")
            .bind(hash_token(token))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn change_admin_password(
        &self,
        username: &str,
        current_password: &str,
        next_password: &str,
    ) -> anyhow::Result<bool> {
        let row = sqlx::query("SELECT password_hash FROM admin_users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(false);
        };
        let password_hash: String = row.try_get("password_hash")?;
        if !verify_password(current_password, &password_hash) {
            return Ok(false);
        }

        let now = unix_timestamp();
        let next_hash = hash_password(next_password)?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query("UPDATE admin_users SET password_hash = ?, updated_at = ? WHERE username = ?")
            .bind(next_hash)
            .bind(now)
            .bind(username)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM admin_sessions WHERE username = ?")
            .bind(username)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'change_admin_password', 'all sessions revoked')",
        )
        .bind(now)
        .bind(username)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn list_admins(&self) -> anyhow::Result<Vec<AdminAccount>> {
        sqlx::query(
            "SELECT username, role, disabled, created_at, updated_at FROM admin_users ORDER BY username",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(AdminAccount {
                username: row.try_get("username")?,
                role: row.try_get("role")?,
                disabled: row.try_get("disabled")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(Into::into)
    }

    pub async fn list_admin_sessions(
        &self,
        username: &str,
        current_token: &str,
    ) -> anyhow::Result<Vec<AdminSessionSummary>> {
        let now = unix_timestamp();
        let current_hash = hash_token(current_token);
        sqlx::query("SELECT substr(token_hash, 1, 24) AS id, token_hash, auth_method, created_at, expires_at, last_used_at FROM admin_sessions WHERE username = ? AND expires_at > ? ORDER BY last_used_at DESC")
            .bind(username)
            .bind(now)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| {
                let token_hash: String = row.try_get("token_hash")?;
                Ok::<_, sqlx::Error>(AdminSessionSummary {
                    id: row.try_get("id")?,
                    auth_method: row.try_get("auth_method")?,
                    created_at: row.try_get("created_at")?,
                    expires_at: row.try_get("expires_at")?,
                    last_used_at: row.try_get("last_used_at")?,
                    current: token_hash == current_hash,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn revoke_admin_session(
        &self,
        actor: &str,
        username: &str,
        session_id: &str,
    ) -> anyhow::Result<bool> {
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "DELETE FROM admin_sessions WHERE username = ? AND substr(token_hash, 1, 24) = ?",
        )
        .bind(username)
        .bind(session_id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() != 1 {
            return Ok(false);
        }
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'admin_session_revoked', ?)")
            .bind(unix_timestamp())
            .bind(actor)
            .bind(format!("session_id={session_id}"))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn create_admin(
        &self,
        actor: &str,
        username: &str,
        password: &str,
        role: &str,
    ) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let result = sqlx::query(
            "INSERT OR IGNORE INTO admin_users (username, password_hash, role, disabled, failed_login_count, user_handle, created_at, updated_at) VALUES (?, ?, ?, 0, 0, ?, ?, ?)",
        )
        .bind(username)
        .bind(hash_password(password)?)
        .bind(role)
        .bind(Uuid::new_v4().to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        self.write_security_audit(
            actor,
            "admin_created",
            &format!("username={username}; role={role}"),
        )
        .await?;
        Ok(true)
    }

    pub async fn set_admin_disabled(
        &self,
        actor: &str,
        username: &str,
        disabled: bool,
    ) -> anyhow::Result<bool> {
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query("SELECT role, disabled FROM admin_users WHERE username = ?")
            .bind(username)
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(row) = row else {
            return Ok(false);
        };
        let role: String = row.try_get("role")?;
        let was_disabled: bool = row.try_get("disabled")?;
        if disabled && !was_disabled && role == "super_admin" {
            let row = sqlx::query(
                "SELECT COUNT(*) AS count FROM admin_users WHERE role = 'super_admin' AND disabled = 0",
            )
            .fetch_one(&mut *transaction)
            .await?;
            if row.try_get::<i64, _>("count")? <= 1 {
                return Ok(false);
            }
        }
        let result =
            sqlx::query("UPDATE admin_users SET disabled = ?, updated_at = ? WHERE username = ?")
                .bind(disabled)
                .bind(unix_timestamp())
                .bind(username)
                .execute(&mut *transaction)
                .await?;
        if disabled {
            sqlx::query("DELETE FROM admin_sessions WHERE username = ?")
                .bind(username)
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'admin_status_changed', ?)",
        )
        .bind(unix_timestamp())
        .bind(actor)
        .bind(format!("username={username}; disabled={disabled}"))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn reset_admin_password(
        &self,
        actor: &str,
        username: &str,
        password: &str,
    ) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE admin_users SET password_hash = ?, failed_login_count = 0, locked_until = NULL, updated_at = ? WHERE username = ?",
        )
        .bind(hash_password(password)?)
        .bind(now)
        .bind(username)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query("DELETE FROM admin_sessions WHERE username = ?")
            .bind(username)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'admin_password_reset', ?)",
        )
        .bind(now)
        .bind(actor)
        .bind(format!("username={username}; all sessions revoked"))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn admin_user_handle(&self, username: &str) -> anyhow::Result<Option<Uuid>> {
        let row =
            sqlx::query("SELECT user_handle FROM admin_users WHERE username = ? AND disabled = 0")
                .bind(username)
                .fetch_optional(&self.pool)
                .await?;
        row.map(|row| {
            let value: String = row.try_get("user_handle")?;
            Uuid::parse_str(&value).context("stored administrator user handle is invalid")
        })
        .transpose()
    }

    pub async fn list_admin_passkeys(
        &self,
        username: &str,
    ) -> anyhow::Result<Vec<AdminPasskeySummary>> {
        sqlx::query(
            "SELECT id, name, created_at, last_used_at FROM admin_passkeys WHERE username = ? ORDER BY created_at, id",
        )
        .bind(username)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(AdminPasskeySummary {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                created_at: row.try_get("created_at")?,
                last_used_at: row.try_get("last_used_at")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(Into::into)
    }

    pub async fn admin_passkeys(&self, username: &str) -> anyhow::Result<Vec<StoredAdminPasskey>> {
        sqlx::query("SELECT id, passkey_json FROM admin_passkeys WHERE username = ?")
            .bind(username)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| {
                let id: i64 = row.try_get("id")?;
                let json: String = row.try_get("passkey_json")?;
                let passkey = serde_json::from_str(&json)
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
                Ok(StoredAdminPasskey { id, passkey })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(Into::into)
    }

    pub async fn add_admin_passkey(
        &self,
        username: &str,
        name: &str,
        passkey: &Passkey,
    ) -> anyhow::Result<bool> {
        let credential_id = serde_json::to_string(passkey.cred_id())?;
        let passkey_json = serde_json::to_string(passkey)?;
        let now = unix_timestamp();
        let result = sqlx::query(
            "INSERT OR IGNORE INTO admin_passkeys (username, name, credential_id, passkey_json, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(username)
        .bind(name)
        .bind(credential_id)
        .bind(passkey_json)
        .bind(now)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 1 {
            self.write_security_audit(username, "admin_passkey_registered", name)
                .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn update_admin_passkey_after_authentication(
        &self,
        username: &str,
        result: &AuthenticationResult,
    ) -> anyhow::Result<bool> {
        let mut passkeys = self.admin_passkeys(username).await?;
        let Some(stored) = passkeys
            .iter_mut()
            .find(|stored| stored.passkey.cred_id() == result.cred_id())
        else {
            return Ok(false);
        };
        if !result.user_verified() {
            return Ok(false);
        }
        stored.passkey.update_credential(result);
        let update = sqlx::query(
            "UPDATE admin_passkeys SET passkey_json = ?, last_used_at = ? WHERE id = ? AND username = ?",
        )
        .bind(serde_json::to_string(&stored.passkey)?)
        .bind(unix_timestamp())
        .bind(stored.id)
        .bind(username)
        .execute(&self.pool)
        .await?;
        Ok(update.rows_affected() == 1)
    }

    pub async fn delete_admin_passkey(&self, username: &str, id: i64) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM admin_passkeys WHERE id = ? AND username = ?")
            .bind(id)
            .bind(username)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 1 {
            self.write_security_audit(
                username,
                "admin_passkey_deleted",
                &format!("passkey_id={id}"),
            )
            .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn admin_passkey_count(&self, username: Option<&str>) -> anyhow::Result<u64> {
        let row = if let Some(username) = username {
            sqlx::query("SELECT COUNT(*) AS count FROM admin_passkeys WHERE username = ?")
                .bind(username)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query("SELECT COUNT(*) AS count FROM admin_passkeys")
                .fetch_one(&self.pool)
                .await?
        };
        Ok(row.try_get::<i64, _>("count")?.max(0) as u64)
    }

    pub async fn admins_without_minimum_passkeys(
        &self,
        minimum: u32,
        except_username: &str,
    ) -> anyhow::Result<Vec<String>> {
        sqlx::query(
            "SELECT u.username FROM admin_users u WHERE u.disabled = 0 AND u.username != ? AND (SELECT COUNT(*) FROM admin_passkeys p WHERE p.username = u.username) < ? ORDER BY u.username",
        )
        .bind(except_username)
        .bind(i64::from(minimum))
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| row.try_get("username"))
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
    }

    pub async fn store_webauthn_challenge(
        &self,
        username: &str,
        kind: &str,
        state_json: &str,
        session_token: Option<&str>,
    ) -> anyhow::Result<String> {
        let challenge = random_secret(48);
        let now = unix_timestamp();
        sqlx::query("DELETE FROM admin_webauthn_challenges WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "INSERT INTO admin_webauthn_challenges (challenge_hash, username, kind, state_json, session_token_hash, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(hash_token(&challenge))
        .bind(username)
        .bind(kind)
        .bind(state_json)
        .bind(session_token.map(hash_token))
        .bind(now)
        .bind(now + WEBAUTHN_CHALLENGE_LIFETIME_SECS)
        .execute(&self.pool)
        .await?;
        Ok(challenge)
    }

    pub async fn take_webauthn_challenge(
        &self,
        challenge: &str,
        kind: &str,
        session_token: Option<&str>,
    ) -> anyhow::Result<Option<(String, String)>> {
        let challenge_hash = hash_token(challenge);
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT username, state_json, session_token_hash, expires_at FROM admin_webauthn_challenges WHERE challenge_hash = ? AND kind = ?",
        )
        .bind(&challenge_hash)
        .bind(kind)
        .fetch_optional(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM admin_webauthn_challenges WHERE challenge_hash = ?")
            .bind(&challenge_hash)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let stored_session: Option<String> = row.try_get("session_token_hash")?;
        let supplied_session = session_token.map(hash_token);
        let expires_at: i64 = row.try_get("expires_at")?;
        if stored_session != supplied_session || expires_at <= unix_timestamp() {
            return Ok(None);
        }
        Ok(Some((row.try_get("username")?, row.try_get("state_json")?)))
    }

    pub async fn is_recent_admin_session(&self, token: &str) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let row = sqlx::query(
            "SELECT COUNT(*) AS count FROM admin_sessions WHERE token_hash = ? AND expires_at > ? AND verified_at >= ?",
        )
        .bind(hash_token(token))
        .bind(now)
        .bind(now - RECENT_ADMIN_VERIFICATION_SECS)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.try_get::<i64, _>("count")? == 1)
    }

    pub async fn create_passkey_session(
        &self,
        username: &str,
        source: &str,
    ) -> anyhow::Result<Option<AdminSession>> {
        let row = sqlx::query("SELECT role FROM admin_users WHERE username = ? AND disabled = 0")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let role: String = row.try_get("role")?;
        let now = unix_timestamp();
        let expires_at = now + SESSION_LIFETIME_SECS;
        let token = random_secret(48);
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO admin_sessions (token_hash, username, auth_method, created_at, expires_at, last_used_at, verified_at) VALUES (?, ?, 'passkey', ?, ?, ?, ?)",
        )
        .bind(hash_token(&token))
        .bind(username)
        .bind(now)
        .bind(expires_at)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'admin_passkey_login_succeeded', ?)",
        )
        .bind(now)
        .bind(username)
        .bind(source)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some(AdminSession {
            token,
            expires_at,
            username: username.to_string(),
            role,
        }))
    }

    pub async fn create_user(
        &self,
        actor: &str,
        email: &str,
        display_name: &str,
        routing_min_length: u8,
    ) -> anyhow::Result<Option<UserAccount>> {
        let email = normalize_email(email);
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let insert = sqlx::query(
            "INSERT OR IGNORE INTO users (email, display_name, disabled, created_at, updated_at) VALUES (?, ?, 0, ?, ?)",
        )
        .bind(&email)
        .bind(display_name)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await?;
        if insert.rows_affected() == 0 {
            return Ok(None);
        }
        let user_id = insert.last_insert_rowid();
        let routing_id =
            insert_unique_routing_id(&mut transaction, user_id, routing_min_length, now).await?;
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'user_created', ?)",
        )
        .bind(now)
        .bind(actor)
        .bind(format!("user_id={user_id}"))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some(UserAccount {
            id: user_id,
            email,
            display_name: display_name.to_string(),
            disabled: false,
            routing_id,
            routing_rotated_at: now,
            created_at: now,
            updated_at: now,
        }))
    }

    pub async fn create_user_with_external_identity(
        &self,
        registration: ExternalRegistration<'_>,
    ) -> anyhow::Result<Option<UserAccount>> {
        let email = normalize_email(registration.email);
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let insert = sqlx::query("INSERT OR IGNORE INTO users (email, display_name, disabled, created_at, updated_at) VALUES (?, ?, 0, ?, ?)")
            .bind(&email)
            .bind(registration.display_name)
            .bind(now)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
        if insert.rows_affected() == 0 {
            return Ok(None);
        }
        let user_id = insert.last_insert_rowid();
        let routing_id = insert_unique_routing_id(
            &mut transaction,
            user_id,
            registration.routing_min_length,
            now,
        )
        .await?;
        sqlx::query("INSERT INTO user_identities (user_id, provider_id, provider_subject, email, email_verified, created_at, updated_at) VALUES (?, ?, ?, ?, 1, ?, ?)")
            .bind(user_id)
            .bind(registration.provider_slug)
            .bind(registration.provider_subject)
            .bind(&email)
            .bind(now)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
        if let Some(invitation_id) = registration.invitation_id {
            let accepted = sqlx::query("UPDATE email_invitations SET status = 'accepted', accepted_at = ? WHERE id = ? AND email = ? AND status = 'pending' AND expires_at > ?")
                .bind(now)
                .bind(invitation_id)
                .bind(&email)
                .bind(now)
                .execute(&mut *transaction)
                .await?;
            if accepted.rows_affected() != 1 {
                anyhow::bail!("email invitation is no longer valid");
            }
        }
        for (action, detail) in [
            ("user_created", format!("user_id={user_id}")),
            (
                "user_identity_bound",
                format!("user_id={user_id}; provider={}", registration.provider_slug),
            ),
        ] {
            sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, ?, ?)")
                .bind(now)
                .bind(registration.actor)
                .bind(action)
                .bind(detail)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(Some(UserAccount {
            id: user_id,
            email,
            display_name: registration.display_name.to_string(),
            disabled: false,
            routing_id,
            routing_rotated_at: now,
            created_at: now,
            updated_at: now,
        }))
    }

    pub async fn list_users(&self) -> anyhow::Result<Vec<UserAccount>> {
        sqlx::query(
            "SELECT u.id, u.email, u.display_name, u.disabled, u.created_at, u.updated_at, r.routing_id, r.created_at AS routing_rotated_at FROM users u JOIN user_routing_ids r ON r.user_id = u.id AND r.active = 1 WHERE u.deleted_at IS NULL ORDER BY u.id",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(user_account_from_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
    }

    pub async fn set_user_disabled(
        &self,
        actor: &str,
        user_id: i64,
        disabled: bool,
    ) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE users SET disabled = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(disabled)
        .bind(now)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        if disabled {
            sqlx::query("DELETE FROM user_sessions WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'user_status_changed', ?)",
        )
        .bind(now)
        .bind(actor)
        .bind(format!("user_id={user_id}; disabled={disabled}"))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn soft_delete_user(&self, actor: &str, user_id: i64) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query("UPDATE users SET disabled = 1, deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
            .bind(now)
            .bind(now)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query("DELETE FROM user_sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE user_routing_ids SET active = 0, revoked_at = ? WHERE user_id = ? AND active = 1")
            .bind(now)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM group_members WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM user_quota_overrides WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'user_soft_deleted', ?)")
            .bind(now)
            .bind(actor)
            .bind(format!("user_id={user_id}"))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn rotate_user_routing_id(
        &self,
        actor: &str,
        user_id: i64,
        routing_min_length: u8,
        cooldown_hours: u32,
        bypass_cooldown: bool,
    ) -> anyhow::Result<RoutingRotationOutcome> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT r.created_at FROM users u JOIN user_routing_ids r ON r.user_id = u.id AND r.active = 1 WHERE u.id = ? AND u.deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(row) = row else {
            return Ok(RoutingRotationOutcome::NotFound);
        };
        let rotated_at: i64 = row.try_get("created_at")?;
        let cooldown_secs = i64::from(cooldown_hours) * 60 * 60;
        if !bypass_cooldown && now < rotated_at.saturating_add(cooldown_secs) {
            return Ok(RoutingRotationOutcome::Cooldown {
                retry_after_secs: rotated_at.saturating_add(cooldown_secs).saturating_sub(now)
                    as u64,
            });
        }
        sqlx::query(
            "UPDATE user_routing_ids SET active = 0, revoked_at = ? WHERE user_id = ? AND active = 1",
        )
        .bind(now)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        let routing_id =
            insert_unique_routing_id(&mut transaction, user_id, routing_min_length, now).await?;
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'user_routing_id_rotated', ?)",
        )
        .bind(now)
        .bind(actor)
        .bind(format!("user_id={user_id}"))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(RoutingRotationOutcome::Rotated { routing_id })
    }

    pub async fn user_by_routing_id(
        &self,
        routing_id: &str,
    ) -> anyhow::Result<Option<UserRoutingIdentity>> {
        sqlx::query(
            "SELECT u.id AS user_id, r.routing_id FROM user_routing_ids r JOIN users u ON u.id = r.user_id WHERE r.routing_id = ? COLLATE NOCASE AND r.active = 1 AND u.disabled = 0 AND u.deleted_at IS NULL",
        )
        .bind(routing_id)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| {
            Ok::<_, sqlx::Error>(UserRoutingIdentity {
                user_id: row.try_get("user_id")?,
                routing_id: row.try_get("routing_id")?,
            })
        })
        .transpose()
        .map_err(Into::into)
    }

    pub async fn authenticate_user_session(
        &self,
        token: &str,
    ) -> anyhow::Result<Option<UserIdentity>> {
        let now = unix_timestamp();
        let token_hash = hash_token(token);
        let row = sqlx::query(
            "SELECT u.id AS user_id, u.email, u.display_name FROM user_sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ? AND s.expires_at > ? AND u.disabled = 0 AND u.deleted_at IS NULL",
        )
        .bind(&token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        if row.is_some() {
            sqlx::query("UPDATE user_sessions SET last_used_at = ? WHERE token_hash = ?")
                .bind(now)
                .bind(token_hash)
                .execute(&self.pool)
                .await?;
        }
        row.map(|row| {
            Ok::<_, sqlx::Error>(UserIdentity {
                user_id: row.try_get("user_id")?,
                email: row.try_get("email")?,
                display_name: row.try_get("display_name")?,
            })
        })
        .transpose()
        .map_err(Into::into)
    }

    pub async fn logout_user(&self, token: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM user_sessions WHERE token_hash = ?")
            .bind(hash_token(token))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn user_account(&self, user_id: i64) -> anyhow::Result<Option<UserAccount>> {
        sqlx::query(
            "SELECT u.id, u.email, u.display_name, u.disabled, u.created_at, u.updated_at, r.routing_id, r.created_at AS routing_rotated_at FROM users u JOIN user_routing_ids r ON r.user_id = u.id AND r.active = 1 WHERE u.id = ? AND u.deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .map(user_account_from_row)
        .transpose()
        .map_err(Into::into)
    }

    pub async fn user_by_email(&self, email: &str) -> anyhow::Result<Option<UserAccount>> {
        sqlx::query(
            "SELECT u.id, u.email, u.display_name, u.disabled, u.created_at, u.updated_at, r.routing_id, r.created_at AS routing_rotated_at FROM users u JOIN user_routing_ids r ON r.user_id = u.id AND r.active = 1 WHERE u.email = ? AND u.deleted_at IS NULL",
        )
        .bind(normalize_email(email))
        .fetch_optional(&self.pool)
        .await?
        .map(user_account_from_row)
        .transpose()
        .map_err(Into::into)
    }

    pub async fn create_user_session(
        &self,
        user_id: i64,
        auth_method: &str,
    ) -> anyhow::Result<Option<UserSession>> {
        let row = sqlx::query(
            "SELECT email, display_name FROM users WHERE id = ? AND disabled = 0 AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let now = unix_timestamp();
        let expires_at = now + USER_SESSION_LIFETIME_SECS;
        let token = random_secret(48);
        sqlx::query("INSERT INTO user_sessions (token_hash, user_id, auth_method, created_at, expires_at, last_used_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(hash_token(&token))
            .bind(user_id)
            .bind(auth_method)
            .bind(now)
            .bind(expires_at)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(Some(UserSession {
            token,
            expires_at,
            identity: UserIdentity {
                user_id,
                email: row.try_get("email")?,
                display_name: row.try_get("display_name")?,
            },
        }))
    }

    pub async fn audit_user_login(&self, user_id: i64, auth_method: &str) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'user_login_succeeded', ?)")
            .bind(unix_timestamp())
            .bind(format!("user:{user_id}"))
            .bind(format!("method={auth_method}"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_auth_providers(&self) -> anyhow::Result<Vec<AuthProvider>> {
        sqlx::query("SELECT id, slug, display_name, kind, preset, enabled, client_id, encrypted_client_secret, issuer_url, authorization_url, token_url, userinfo_url, emails_url, scopes_json, subject_field, email_field, email_verified_field, display_name_field, allow_registration, auto_link_by_email FROM auth_providers ORDER BY display_name")
            .fetch_all(&self.pool).await?.into_iter().map(auth_provider_from_row).collect()
    }

    pub async fn auth_provider_by_slug(&self, slug: &str) -> anyhow::Result<Option<AuthProvider>> {
        sqlx::query("SELECT id, slug, display_name, kind, preset, enabled, client_id, encrypted_client_secret, issuer_url, authorization_url, token_url, userinfo_url, emails_url, scopes_json, subject_field, email_field, email_verified_field, display_name_field, allow_registration, auto_link_by_email FROM auth_providers WHERE slug = ? COLLATE NOCASE")
            .bind(slug).fetch_optional(&self.pool).await?.map(auth_provider_from_row).transpose()
    }

    pub async fn auth_provider_by_id(&self, id: i64) -> anyhow::Result<Option<AuthProvider>> {
        sqlx::query("SELECT id, slug, display_name, kind, preset, enabled, client_id, encrypted_client_secret, issuer_url, authorization_url, token_url, userinfo_url, emails_url, scopes_json, subject_field, email_field, email_verified_field, display_name_field, allow_registration, auto_link_by_email FROM auth_providers WHERE id = ?")
            .bind(id).fetch_optional(&self.pool).await?.map(auth_provider_from_row).transpose()
    }

    pub async fn save_auth_provider(
        &self,
        actor: &str,
        provider: &AuthProvider,
        preserve_secret: bool,
    ) -> anyhow::Result<i64> {
        let now = unix_timestamp();
        let encrypted_secret = if preserve_secret && provider.id != 0 {
            sqlx::query("SELECT encrypted_client_secret FROM auth_providers WHERE id = ?")
                .bind(provider.id)
                .fetch_optional(&self.pool)
                .await?
                .map(|row| row.try_get::<Option<String>, _>("encrypted_client_secret"))
                .transpose()?
                .flatten()
        } else {
            provider.encrypted_client_secret.clone()
        };
        let scopes = serde_json::to_string(&provider.scopes)?;
        let mut transaction = self.pool.begin().await?;
        let id = if provider.id == 0 {
            sqlx::query("INSERT INTO auth_providers (slug, display_name, kind, preset, enabled, client_id, encrypted_client_secret, issuer_url, authorization_url, token_url, userinfo_url, emails_url, scopes_json, subject_field, email_field, email_verified_field, display_name_field, allow_registration, auto_link_by_email, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&provider.slug).bind(&provider.display_name).bind(&provider.kind).bind(&provider.preset).bind(provider.enabled).bind(&provider.client_id).bind(&encrypted_secret).bind(&provider.issuer_url).bind(&provider.authorization_url).bind(&provider.token_url).bind(&provider.userinfo_url).bind(&provider.emails_url).bind(&scopes).bind(&provider.subject_field).bind(&provider.email_field).bind(&provider.email_verified_field).bind(&provider.display_name_field).bind(provider.allow_registration).bind(provider.auto_link_by_email).bind(now).bind(now).execute(&mut *transaction).await?.last_insert_rowid()
        } else {
            let previous_slug: String =
                sqlx::query_scalar("SELECT slug FROM auth_providers WHERE id = ?")
                    .bind(provider.id)
                    .fetch_optional(&mut *transaction)
                    .await?
                    .context("authentication provider not found")?;
            let result = sqlx::query("UPDATE auth_providers SET slug=?, display_name=?, kind=?, preset=?, enabled=?, client_id=?, encrypted_client_secret=?, issuer_url=?, authorization_url=?, token_url=?, userinfo_url=?, emails_url=?, scopes_json=?, subject_field=?, email_field=?, email_verified_field=?, display_name_field=?, allow_registration=?, auto_link_by_email=?, updated_at=? WHERE id=?")
                .bind(&provider.slug).bind(&provider.display_name).bind(&provider.kind).bind(&provider.preset).bind(provider.enabled).bind(&provider.client_id).bind(&encrypted_secret).bind(&provider.issuer_url).bind(&provider.authorization_url).bind(&provider.token_url).bind(&provider.userinfo_url).bind(&provider.emails_url).bind(&scopes).bind(&provider.subject_field).bind(&provider.email_field).bind(&provider.email_verified_field).bind(&provider.display_name_field).bind(provider.allow_registration).bind(provider.auto_link_by_email).bind(now).bind(provider.id).execute(&mut *transaction).await?;
            if result.rows_affected() == 0 {
                anyhow::bail!("authentication provider not found");
            }
            if previous_slug != provider.slug {
                sqlx::query("UPDATE user_identities SET provider_id = ?, updated_at = ? WHERE provider_id = ? COLLATE NOCASE")
                    .bind(&provider.slug)
                    .bind(now)
                    .bind(previous_slug)
                    .execute(&mut *transaction)
                    .await?;
            }
            provider.id
        };
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'auth_provider_saved', ?)")
            .bind(now).bind(actor).bind(format!("provider_id={id}")).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(id)
    }

    pub async fn delete_auth_provider(&self, actor: &str, id: i64) -> anyhow::Result<bool> {
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query("DELETE FROM auth_providers WHERE id = ? AND NOT EXISTS (SELECT 1 FROM user_identities WHERE provider_id = auth_providers.slug COLLATE NOCASE)")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'auth_provider_deleted', ?)")
            .bind(unix_timestamp()).bind(actor).bind(format!("provider_id={id}")).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn auth_provider_identity_count(&self, id: i64) -> anyhow::Result<u64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_identities i JOIN auth_providers p ON p.slug = i.provider_id COLLATE NOCASE WHERE p.id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count as u64)
    }

    pub async fn store_user_auth_flow(
        &self,
        state: &str,
        provider_id: i64,
        encrypted_payload: &str,
        mode: &str,
        user_id: Option<i64>,
        expires_at: i64,
    ) -> anyhow::Result<()> {
        let now = unix_timestamp();
        sqlx::query("DELETE FROM user_auth_flows WHERE expires_at <= ? OR used_at IS NOT NULL")
            .bind(now)
            .execute(&self.pool)
            .await?;
        sqlx::query("INSERT INTO user_auth_flows (state_hash, provider_id, encrypted_payload, mode, user_id, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(hash_token(state)).bind(provider_id).bind(encrypted_payload).bind(mode).bind(user_id).bind(expires_at).bind(now).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn take_user_auth_flow(&self, state: &str) -> anyhow::Result<Option<StoredAuthFlow>> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query("SELECT provider_id, encrypted_payload, mode, user_id FROM user_auth_flows WHERE state_hash = ? AND expires_at > ? AND used_at IS NULL")
            .bind(hash_token(state)).bind(now).fetch_optional(&mut *transaction).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let updated = sqlx::query(
            "UPDATE user_auth_flows SET used_at = ? WHERE state_hash = ? AND used_at IS NULL",
        )
        .bind(now)
        .bind(hash_token(state))
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() != 1 {
            return Ok(None);
        }
        transaction.commit().await?;
        Ok(Some(StoredAuthFlow {
            provider_id: row.try_get("provider_id")?,
            encrypted_payload: row.try_get("encrypted_payload")?,
            mode: row.try_get("mode")?,
            user_id: row.try_get("user_id")?,
        }))
    }

    pub async fn user_by_external_identity(
        &self,
        provider_slug: &str,
        subject: &str,
    ) -> anyhow::Result<Option<UserAccount>> {
        sqlx::query("SELECT u.id, u.email, u.display_name, u.disabled, u.created_at, u.updated_at, r.routing_id, r.created_at AS routing_rotated_at FROM user_identities i JOIN users u ON u.id = i.user_id JOIN user_routing_ids r ON r.user_id = u.id AND r.active = 1 WHERE i.provider_id = ? COLLATE NOCASE AND i.provider_subject = ? AND u.deleted_at IS NULL")
            .bind(provider_slug).bind(subject).fetch_optional(&self.pool).await?.map(user_account_from_row).transpose().map_err(Into::into)
    }

    pub async fn bind_external_identity(
        &self,
        actor: &str,
        user_id: i64,
        provider_slug: &str,
        subject: &str,
        email: Option<&str>,
        email_verified: bool,
    ) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query("INSERT OR IGNORE INTO user_identities (user_id, provider_id, provider_subject, email, email_verified, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(user_id).bind(provider_slug).bind(subject).bind(email.map(normalize_email)).bind(email_verified).bind(now).bind(now).execute(&mut *transaction).await?;
        if result.rows_affected() == 0 {
            let owner = sqlx::query("SELECT user_id FROM user_identities WHERE provider_id = ? COLLATE NOCASE AND provider_subject = ?")
                .bind(provider_slug).bind(subject).fetch_one(&mut *transaction).await?.try_get::<i64, _>("user_id")?;
            return Ok(owner == user_id);
        }
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'user_identity_bound', ?)")
            .bind(now).bind(actor).bind(format!("user_id={user_id}; provider={provider_slug}")).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn list_external_identities(
        &self,
        user_id: i64,
    ) -> anyhow::Result<Vec<ExternalUserIdentity>> {
        sqlx::query("SELECT i.id, i.provider_id, COALESCE(p.display_name, i.provider_id) AS provider_name, i.provider_subject, i.email, i.email_verified, i.created_at FROM user_identities i LEFT JOIN auth_providers p ON p.slug = i.provider_id COLLATE NOCASE WHERE i.user_id = ? ORDER BY i.created_at")
            .bind(user_id).fetch_all(&self.pool).await?.into_iter().map(|row| Ok::<_, sqlx::Error>(ExternalUserIdentity { id: row.try_get("id")?, provider_slug: row.try_get("provider_id")?, provider_name: row.try_get("provider_name")?, provider_subject: row.try_get("provider_subject")?, email: row.try_get("email")?, email_verified: row.try_get("email_verified")?, created_at: row.try_get("created_at")? })).collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub async fn delete_external_identity(
        &self,
        actor: &str,
        user_id: i64,
        identity_id: i64,
    ) -> anyhow::Result<bool> {
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query("DELETE FROM user_identities WHERE id = ? AND user_id = ?")
            .bind(identity_id)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'user_identity_unbound', ?)")
            .bind(unix_timestamp()).bind(actor).bind(format!("user_id={user_id}; identity_id={identity_id}")).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn external_identity_count(&self, user_id: i64) -> anyhow::Result<u64> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM user_identities WHERE user_id = ?")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count as u64)
    }

    pub async fn smtp_settings(&self) -> anyhow::Result<Option<SmtpSettings>> {
        sqlx::query("SELECT enabled, host, port, security, username, encrypted_password, from_name, from_address FROM smtp_settings WHERE singleton = 1")
            .fetch_optional(&self.pool)
            .await?
            .map(|row| {
                Ok::<_, sqlx::Error>(SmtpSettings {
                    enabled: row.try_get("enabled")?,
                    host: row.try_get("host")?,
                    port: row.try_get::<i64, _>("port")? as u16,
                    security: row.try_get("security")?,
                    username: row.try_get("username")?,
                    encrypted_password: row.try_get("encrypted_password")?,
                    from_name: row.try_get("from_name")?,
                    from_address: row.try_get("from_address")?,
                })
            })
            .transpose()
            .map_err(Into::into)
    }

    pub async fn save_smtp_settings(
        &self,
        actor: &str,
        settings: &SmtpSettings,
        preserve_password: bool,
    ) -> anyhow::Result<()> {
        let now = unix_timestamp();
        let encrypted_password = if preserve_password {
            self.smtp_settings()
                .await?
                .and_then(|current| current.encrypted_password)
        } else {
            settings.encrypted_password.clone()
        };
        let mut transaction = self.pool.begin().await?;
        sqlx::query("INSERT INTO smtp_settings (singleton, enabled, host, port, security, username, encrypted_password, from_name, from_address, updated_at) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(singleton) DO UPDATE SET enabled=excluded.enabled, host=excluded.host, port=excluded.port, security=excluded.security, username=excluded.username, encrypted_password=excluded.encrypted_password, from_name=excluded.from_name, from_address=excluded.from_address, updated_at=excluded.updated_at")
            .bind(settings.enabled)
            .bind(&settings.host)
            .bind(i64::from(settings.port))
            .bind(&settings.security)
            .bind(&settings.username)
            .bind(encrypted_password)
            .bind(&settings.from_name)
            .bind(&settings.from_address)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'smtp_settings_updated', 'smtp_settings')")
            .bind(now)
            .bind(actor)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn create_email_invitation(
        &self,
        actor: &str,
        email: &str,
        display_name: &str,
        token: &str,
        expires_at: i64,
    ) -> anyhow::Result<i64> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query("INSERT INTO email_invitations (email, display_name, token_hash, status, expires_at, created_at) VALUES (?, ?, ?, 'pending', ?, ?)")
            .bind(normalize_email(email))
            .bind(display_name)
            .bind(hash_token(token))
            .bind(expires_at)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
        let id = result.last_insert_rowid();
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'email_invitation_created', ?)")
            .bind(now)
            .bind(actor)
            .bind(format!("invitation_id={id}"))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(id)
    }

    pub async fn list_email_invitations(&self) -> anyhow::Result<Vec<EmailInvitation>> {
        sqlx::query("SELECT id, email, display_name, status, expires_at, created_at FROM email_invitations ORDER BY id DESC")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| Ok(EmailInvitation {
                id: row.try_get("id")?, email: row.try_get("email")?, display_name: row.try_get("display_name")?, status: row.try_get("status")?, expires_at: row.try_get("expires_at")?, created_at: row.try_get("created_at")?,
            }))
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(Into::into)
    }

    pub async fn email_invitation(&self, id: i64) -> anyhow::Result<Option<EmailInvitation>> {
        sqlx::query("SELECT id, email, display_name, status, expires_at, created_at FROM email_invitations WHERE id = ?")
            .bind(id).fetch_optional(&self.pool).await?
            .map(|row| Ok::<_, sqlx::Error>(EmailInvitation { id: row.try_get("id")?, email: row.try_get("email")?, display_name: row.try_get("display_name")?, status: row.try_get("status")?, expires_at: row.try_get("expires_at")?, created_at: row.try_get("created_at")? }))
            .transpose().map_err(Into::into)
    }

    pub async fn revoke_email_invitation(&self, actor: &str, id: i64) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let result = sqlx::query("UPDATE email_invitations SET status = 'revoked', revoked_at = ? WHERE id = ? AND status = 'pending'")
            .bind(now).bind(id).execute(&self.pool).await?;
        if result.rows_affected() == 1 {
            self.write_security_audit(
                actor,
                "email_invitation_revoked",
                &format!("invitation_id={id}"),
            )
            .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn renew_email_invitation(
        &self,
        actor: &str,
        id: i64,
        token: &str,
        expires_at: i64,
    ) -> anyhow::Result<Option<EmailInvitation>> {
        let result = sqlx::query("UPDATE email_invitations SET token_hash = ?, expires_at = ? WHERE id = ? AND status = 'pending'")
            .bind(hash_token(token)).bind(expires_at).bind(id).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.write_security_audit(
            actor,
            "email_invitation_resent",
            &format!("invitation_id={id}"),
        )
        .await?;
        self.email_invitation(id).await
    }

    pub async fn valid_invitation(&self, email: &str, token: &str) -> anyhow::Result<Option<i64>> {
        sqlx::query("SELECT id FROM email_invitations WHERE email = ? AND token_hash = ? AND status = 'pending' AND expires_at > ?")
            .bind(normalize_email(email)).bind(hash_token(token)).bind(unix_timestamp())
            .fetch_optional(&self.pool).await?
            .map(|row| row.try_get("id")).transpose().map_err(Into::into)
    }

    pub async fn store_email_login_token(
        &self,
        email: &str,
        token: &str,
        code: &str,
        invitation_id: Option<i64>,
        expires_at: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE email_login_tokens SET used_at = ? WHERE email = ? AND used_at IS NULL",
        )
        .bind(unix_timestamp())
        .bind(normalize_email(email))
        .execute(&self.pool)
        .await?;
        sqlx::query("INSERT INTO email_login_tokens (email, token_hash, code_hash, invitation_id, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(normalize_email(email)).bind(hash_token(token)).bind(hash_token(code)).bind(invitation_id).bind(expires_at).bind(unix_timestamp())
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn consume_email_login_token(
        &self,
        email: &str,
        credential: &str,
        is_code: bool,
    ) -> anyhow::Result<Option<Option<i64>>> {
        let email = normalize_email(email);
        let column = if is_code { "code_hash" } else { "token_hash" };
        let query = format!("SELECT id, invitation_id FROM email_login_tokens WHERE email = ? AND {column} = ? AND used_at IS NULL AND expires_at > ? AND attempts < 5 ORDER BY id DESC LIMIT 1");
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(&query)
            .bind(&email)
            .bind(hash_token(credential))
            .bind(unix_timestamp())
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(row) = row else {
            if is_code {
                sqlx::query("UPDATE email_login_tokens SET attempts = attempts + 1 WHERE id = (SELECT id FROM email_login_tokens WHERE email = ? AND used_at IS NULL ORDER BY id DESC LIMIT 1)")
                    .bind(&email).execute(&mut *transaction).await?;
            }
            transaction.commit().await?;
            return Ok(None);
        };
        let id: i64 = row.try_get("id")?;
        let invitation_id: Option<i64> = row.try_get("invitation_id")?;
        let result = sqlx::query(
            "UPDATE email_login_tokens SET used_at = ? WHERE id = ? AND used_at IS NULL",
        )
        .bind(unix_timestamp())
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok((result.rows_affected() == 1).then_some(invitation_id))
    }

    pub async fn accept_email_invitation(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE email_invitations SET status = 'accepted', accepted_at = ? WHERE id = ? AND status = 'pending'")
            .bind(unix_timestamp()).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn enqueue_email(
        &self,
        recipient: &str,
        subject: &str,
        encrypted_body: &str,
    ) -> anyhow::Result<i64> {
        let now = unix_timestamp();
        let result = sqlx::query("INSERT INTO email_outbox (recipient, subject, encrypted_body, next_attempt_at, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(normalize_email(recipient)).bind(subject).bind(encrypted_body).bind(now).bind(now).execute(&self.pool).await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn allow_email_send(&self, email: &str, source: &str) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let window_secs = 10 * 60;
        let keys = [
            (format!("email:{}", normalize_email(email)), 3_i64),
            (format!("source:{source}"), 10_i64),
            ("instance".to_string(), 50_i64),
        ];
        let mut transaction = self.pool.begin().await?;
        for (key, limit) in &keys {
            let row = sqlx::query(
                "SELECT window_start, request_count FROM email_rate_limits WHERE limit_key = ?",
            )
            .bind(key)
            .fetch_optional(&mut *transaction)
            .await?;
            if let Some(row) = row {
                let window_start: i64 = row.try_get("window_start")?;
                let count: i64 = row.try_get("request_count")?;
                if now < window_start + window_secs && count >= *limit {
                    return Ok(false);
                }
            }
        }
        for (key, _) in keys {
            sqlx::query("INSERT INTO email_rate_limits (limit_key, window_start, request_count) VALUES (?, ?, 1) ON CONFLICT(limit_key) DO UPDATE SET window_start = CASE WHEN email_rate_limits.window_start + ? <= ? THEN ? ELSE email_rate_limits.window_start END, request_count = CASE WHEN email_rate_limits.window_start + ? <= ? THEN 1 ELSE email_rate_limits.request_count + 1 END")
                .bind(key)
                .bind(now)
                .bind(window_secs)
                .bind(now)
                .bind(now)
                .bind(window_secs)
                .bind(now)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn pending_outbox(&self, limit: u32) -> anyhow::Result<Vec<OutboxMessage>> {
        sqlx::query("SELECT id, recipient, subject, encrypted_body, attempts FROM email_outbox WHERE status = 'pending' AND next_attempt_at <= ? ORDER BY id LIMIT ?")
            .bind(unix_timestamp()).bind(i64::from(limit)).fetch_all(&self.pool).await?
            .into_iter().map(|row| Ok(OutboxMessage { id: row.try_get("id")?, recipient: row.try_get("recipient")?, subject: row.try_get("subject")?, encrypted_body: row.try_get("encrypted_body")?, attempts: row.try_get::<i64, _>("attempts")? as u32 }))
            .collect::<Result<Vec<_>, sqlx::Error>>().map_err(Into::into)
    }

    pub async fn mark_outbox_sent(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE email_outbox SET status = 'sent', sent_at = ? WHERE id = ?")
            .bind(unix_timestamp())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_outbox_failed(
        &self,
        id: i64,
        attempts: u32,
        error: &str,
    ) -> anyhow::Result<()> {
        let terminal = attempts >= 5;
        let delay = 30_i64.saturating_mul(1_i64 << attempts.min(6));
        sqlx::query("UPDATE email_outbox SET status = ?, attempts = ?, next_attempt_at = ?, last_error = ? WHERE id = ?")
            .bind(if terminal { "failed" } else { "pending" })
            .bind(i64::from(attempts)).bind(unix_timestamp() + delay)
            .bind(error.chars().take(200).collect::<String>()).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn write_security_audit(
        &self,
        username: &str,
        action: &str,
        detail: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, ?, ?)",
        )
        .bind(unix_timestamp())
        .bind(if username.is_empty() { "unknown" } else { username })
        .bind(action)
        .bind(detail)
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

    pub async fn list_billing_groups(&self) -> anyhow::Result<Vec<BillingGroup>> {
        sqlx::query(
            "SELECT g.id, g.name, q.monthly_limit_bytes, COUNT(m.user_id) AS member_count FROM groups g LEFT JOIN group_quota_settings q ON q.group_id = g.id LEFT JOIN group_members m ON m.group_id = g.id AND m.is_billing = 1 WHERE g.kind = 'billing' GROUP BY g.id, g.name, q.monthly_limit_bytes ORDER BY g.name",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok::<_, sqlx::Error>(BillingGroup {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                monthly_limit_bytes: row
                    .try_get::<Option<i64>, _>("monthly_limit_bytes")?
                    .map(as_u64),
                member_count: as_u64(row.try_get("member_count")?),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
    }

    pub async fn create_billing_group(
        &self,
        actor: &str,
        name: &str,
        monthly_limit_bytes: Option<u64>,
    ) -> anyhow::Result<Option<BillingGroup>> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let insert = sqlx::query(
            "INSERT OR IGNORE INTO groups (name, kind, created_at, updated_at) VALUES (?, 'billing', ?, ?)",
        )
        .bind(name)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await?;
        if insert.rows_affected() == 0 {
            return Ok(None);
        }
        let id = insert.last_insert_rowid();
        sqlx::query("INSERT INTO group_quota_settings (group_id, monthly_limit_bytes, updated_at) VALUES (?, ?, ?)")
            .bind(id)
            .bind(monthly_limit_bytes.map(limit_to_i64))
            .bind(now)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'billing_group_created', ?)")
            .bind(now).bind(actor).bind(format!("group_id={id}")).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(Some(BillingGroup {
            id,
            name: name.to_string(),
            monthly_limit_bytes,
            member_count: 0,
        }))
    }

    pub async fn update_billing_group(
        &self,
        actor: &str,
        id: i64,
        name: &str,
        monthly_limit_bytes: Option<u64>,
    ) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let update = sqlx::query(
            "UPDATE groups SET name = ?, updated_at = ? WHERE id = ? AND kind = 'billing'",
        )
        .bind(name)
        .bind(now)
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        if update.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query("INSERT INTO group_quota_settings (group_id, monthly_limit_bytes, updated_at) VALUES (?, ?, ?) ON CONFLICT(group_id) DO UPDATE SET monthly_limit_bytes = excluded.monthly_limit_bytes, updated_at = excluded.updated_at")
            .bind(id).bind(monthly_limit_bytes.map(limit_to_i64)).bind(now).execute(&mut *transaction).await?;
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'billing_group_updated', ?)")
            .bind(now).bind(actor).bind(format!("group_id={id}")).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn set_user_billing_profile(
        &self,
        actor: &str,
        user_id: i64,
        group_id: Option<i64>,
        quota_mode: &str,
        user_monthly_limit_bytes: Option<u64>,
    ) -> anyhow::Result<bool> {
        let now = unix_timestamp();
        let mut transaction = self.pool.begin().await?;
        let user_exists = sqlx::query("SELECT 1 FROM users WHERE id = ? AND deleted_at IS NULL")
            .bind(user_id)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some();
        if !user_exists {
            return Ok(false);
        }
        if let Some(group_id) = group_id {
            let group_exists =
                sqlx::query("SELECT 1 FROM groups WHERE id = ? AND kind = 'billing'")
                    .bind(group_id)
                    .fetch_optional(&mut *transaction)
                    .await?
                    .is_some();
            if !group_exists {
                return Ok(false);
            }
        }
        sqlx::query("DELETE FROM group_members WHERE user_id = ? AND is_billing = 1")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        if let Some(group_id) = group_id {
            sqlx::query("INSERT INTO group_members (group_id, user_id, is_billing, created_at) VALUES (?, ?, 1, ?) ON CONFLICT(group_id, user_id) DO UPDATE SET is_billing = 1")
                .bind(group_id).bind(user_id).bind(now).execute(&mut *transaction).await?;
        }
        if quota_mode == "default" {
            sqlx::query("DELETE FROM user_quota_overrides WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
        } else {
            sqlx::query("INSERT INTO user_quota_overrides (user_id, mode, monthly_limit_bytes, updated_at) VALUES (?, ?, ?, ?) ON CONFLICT(user_id) DO UPDATE SET mode = excluded.mode, monthly_limit_bytes = excluded.monthly_limit_bytes, updated_at = excluded.updated_at")
                .bind(user_id).bind(quota_mode).bind(user_monthly_limit_bytes.map(limit_to_i64)).bind(now).execute(&mut *transaction).await?;
        }
        sqlx::query("INSERT INTO config_audit_log (created_at, username, action, detail) VALUES (?, ?, 'user_billing_updated', ?)")
            .bind(now).bind(actor).bind(format!("user_id={user_id}")).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn user_billing_profile(
        &self,
        user_id: i64,
    ) -> anyhow::Result<Option<UserBillingProfile>> {
        let row = sqlx::query(
            "SELECT u.id AS user_id, g.id AS group_id, g.name AS group_name, gq.monthly_limit_bytes AS group_limit, uq.mode AS quota_mode, uq.monthly_limit_bytes AS user_limit FROM users u LEFT JOIN group_members gm ON gm.user_id = u.id AND gm.is_billing = 1 LEFT JOIN groups g ON g.id = gm.group_id LEFT JOIN group_quota_settings gq ON gq.group_id = g.id LEFT JOIN user_quota_overrides uq ON uq.user_id = u.id WHERE u.id = ? AND u.deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok::<_, sqlx::Error>(UserBillingProfile {
                user_id: row.try_get("user_id")?,
                group_id: row.try_get("group_id")?,
                group_name: row.try_get("group_name")?,
                group_monthly_limit_bytes: row
                    .try_get::<Option<i64>, _>("group_limit")?
                    .map(as_u64),
                quota_mode: row
                    .try_get::<Option<String>, _>("quota_mode")?
                    .unwrap_or_else(|| "default".to_string()),
                user_monthly_limit_bytes: row.try_get::<Option<i64>, _>("user_limit")?.map(as_u64),
            })
        })
        .transpose()
        .map_err(Into::into)
    }

    pub async fn try_reserve_hierarchical_bytes(
        &self,
        month: &str,
        user_id: i64,
        global_limit: Option<u64>,
        default_user_limit: Option<u64>,
        bytes: u64,
    ) -> anyhow::Result<HierarchicalReservationOutcome> {
        let bytes = limit_to_i64(bytes);
        let mut transaction = self.pool.begin().await?;
        let profile = sqlx::query(
            "SELECT g.id AS group_id, gq.monthly_limit_bytes AS group_limit, uq.mode AS quota_mode, uq.monthly_limit_bytes AS user_limit FROM users u LEFT JOIN group_members gm ON gm.user_id = u.id AND gm.is_billing = 1 LEFT JOIN groups g ON g.id = gm.group_id LEFT JOIN group_quota_settings gq ON gq.group_id = g.id LEFT JOIN user_quota_overrides uq ON uq.user_id = u.id WHERE u.id = ? AND u.disabled = 0 AND u.deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(profile) = profile else {
            return Ok(HierarchicalReservationOutcome::Exceeded { scope: "user" });
        };
        let group_id: Option<i64> = profile.try_get("group_id")?;
        let group_limit = profile.try_get::<Option<i64>, _>("group_limit")?;
        let quota_mode = profile.try_get::<Option<String>, _>("quota_mode")?;
        let override_limit = profile.try_get::<Option<i64>, _>("user_limit")?;
        let user_limit = match quota_mode.as_deref() {
            Some("unlimited") => None,
            Some("custom") => override_limit,
            _ => default_user_limit.map(limit_to_i64),
        };

        sqlx::query("INSERT OR IGNORE INTO traffic_monthly (month) VALUES (?)")
            .bind(month)
            .execute(&mut *transaction)
            .await?;
        if let Some(limit) = global_limit.map(limit_to_i64) {
            let result = sqlx::query("UPDATE traffic_monthly SET reserved_bytes = reserved_bytes + ? WHERE month = ? AND response_bytes + reserved_bytes + ? <= ?")
                .bind(bytes).bind(month).bind(bytes).bind(limit).execute(&mut *transaction).await?;
            if result.rows_affected() == 0 {
                return Ok(HierarchicalReservationOutcome::Exceeded { scope: "global" });
            }
        }

        sqlx::query("INSERT OR IGNORE INTO user_traffic_monthly (month, user_id) VALUES (?, ?)")
            .bind(month)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        let user_result = if let Some(limit) = user_limit {
            sqlx::query("UPDATE user_traffic_monthly SET reserved_bytes = reserved_bytes + ? WHERE month = ? AND user_id = ? AND response_bytes + reserved_bytes + ? <= ?")
                .bind(bytes).bind(month).bind(user_id).bind(bytes).bind(limit).execute(&mut *transaction).await?
        } else {
            sqlx::query("UPDATE user_traffic_monthly SET reserved_bytes = reserved_bytes + ? WHERE month = ? AND user_id = ?")
                .bind(bytes).bind(month).bind(user_id).execute(&mut *transaction).await?
        };
        if user_result.rows_affected() == 0 {
            return Ok(HierarchicalReservationOutcome::Exceeded { scope: "user" });
        }

        if let Some(group_id) = group_id {
            sqlx::query(
                "INSERT OR IGNORE INTO group_traffic_monthly (month, group_id) VALUES (?, ?)",
            )
            .bind(month)
            .bind(group_id)
            .execute(&mut *transaction)
            .await?;
            let group_result = if let Some(limit) = group_limit {
                sqlx::query("UPDATE group_traffic_monthly SET reserved_bytes = reserved_bytes + ? WHERE month = ? AND group_id = ? AND response_bytes + reserved_bytes + ? <= ?")
                    .bind(bytes).bind(month).bind(group_id).bind(bytes).bind(limit).execute(&mut *transaction).await?
            } else {
                sqlx::query("UPDATE group_traffic_monthly SET reserved_bytes = reserved_bytes + ? WHERE month = ? AND group_id = ?")
                    .bind(bytes).bind(month).bind(group_id).execute(&mut *transaction).await?
            };
            if group_result.rows_affected() == 0 {
                return Ok(HierarchicalReservationOutcome::Exceeded { scope: "group" });
            }
        }
        transaction.commit().await?;
        Ok(HierarchicalReservationOutcome::Reserved { group_id })
    }

    pub async fn try_reserve_monthly_bytes(
        &self,
        month: &str,
        limit: u64,
        bytes: u64,
    ) -> anyhow::Result<bool> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("INSERT OR IGNORE INTO traffic_monthly (month) VALUES (?)")
            .bind(month)
            .execute(&mut *transaction)
            .await?;
        let result = sqlx::query("UPDATE traffic_monthly SET reserved_bytes = reserved_bytes + ? WHERE month = ? AND response_bytes + reserved_bytes + ? <= ?")
            .bind(i64::try_from(bytes).unwrap_or(i64::MAX)).bind(month).bind(i64::try_from(bytes).unwrap_or(i64::MAX)).bind(i64::try_from(limit).unwrap_or(i64::MAX)).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn record_proxy_response(
        &self,
        record: ProxyTrafficRecord<'_>,
    ) -> anyhow::Result<()> {
        let bytes = i64::try_from(record.response_bytes).unwrap_or(i64::MAX);
        let reserved = i64::try_from(record.reserved_bytes).unwrap_or(i64::MAX);
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
            "INSERT INTO traffic_monthly (month, request_count, response_bytes, upstream_bytes, error_count, quota_exceeded) VALUES (?, 1, ?, 0, ?, 0) ON CONFLICT(month) DO UPDATE SET request_count = request_count + 1, response_bytes = response_bytes + excluded.response_bytes, reserved_bytes = MAX(0, reserved_bytes - ?), error_count = error_count + excluded.error_count",
        )
        .bind(record.month)
        .bind(bytes)
        .bind(errors)
        .bind(reserved)
        .execute(&mut *transaction)
        .await?;
        if let Some(user_id) = record.user_id {
            sqlx::query("INSERT INTO user_traffic_daily (day, user_id, target_code, request_count, response_bytes, error_count) VALUES (?, ?, ?, 1, ?, ?) ON CONFLICT(day, user_id, target_code) DO UPDATE SET request_count = request_count + 1, response_bytes = response_bytes + excluded.response_bytes, error_count = error_count + excluded.error_count")
                .bind(record.day).bind(user_id).bind(record.target_code).bind(bytes).bind(errors).execute(&mut *transaction).await?;
            sqlx::query("INSERT INTO user_traffic_monthly (month, user_id, request_count, response_bytes, error_count) VALUES (?, ?, 1, ?, ?) ON CONFLICT(month, user_id) DO UPDATE SET request_count = request_count + 1, response_bytes = response_bytes + excluded.response_bytes, error_count = error_count + excluded.error_count, reserved_bytes = MAX(0, reserved_bytes - ?)")
                .bind(record.month).bind(user_id).bind(bytes).bind(errors).bind(reserved).execute(&mut *transaction).await?;
        }
        if let Some(group_id) = record.group_id {
            sqlx::query("INSERT INTO group_traffic_monthly (month, group_id, request_count, response_bytes, error_count) VALUES (?, ?, 1, ?, ?) ON CONFLICT(month, group_id) DO UPDATE SET request_count = request_count + 1, response_bytes = response_bytes + excluded.response_bytes, error_count = error_count + excluded.error_count, reserved_bytes = MAX(0, reserved_bytes - ?)")
                .bind(record.month).bind(group_id).bind(bytes).bind(errors).bind(reserved).execute(&mut *transaction).await?;
        }
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
        let cutoff = now.saturating_sub(
            i64::from(record.request_event_retention_days).saturating_mul(24 * 60 * 60),
        );
        sqlx::query("DELETE FROM request_events WHERE created_at < ?")
            .bind(cutoff)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn mark_month_quota_exceeded(&self, month: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO traffic_monthly (month, quota_exceeded) VALUES (?, 1) ON CONFLICT(month) DO UPDATE SET quota_exceeded = 1",
        )
        .bind(month)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn traffic_overview(&self, month: &str) -> anyhow::Result<TrafficOverview> {
        let row = sqlx::query(
            "SELECT request_count, response_bytes, error_count, quota_exceeded FROM traffic_monthly WHERE month = ?",
        )
        .bind(month)
        .fetch_optional(&self.pool)
        .await?;
        let (request_count, response_bytes, error_count, quota_exceeded) = row
            .map(|row| {
                Ok::<_, sqlx::Error>((
                    row.try_get::<i64, _>("request_count")?,
                    row.try_get::<i64, _>("response_bytes")?,
                    row.try_get::<i64, _>("error_count")?,
                    row.try_get::<i64, _>("quota_exceeded")?,
                ))
            })
            .transpose()?
            .unwrap_or((0, 0, 0, 0));
        let month_days = format!("{month}-%");
        let daily = sqlx::query(
            "SELECT day, target_code, request_count, response_bytes, error_count FROM traffic_daily WHERE day LIKE ? ORDER BY day ASC, target_code ASC",
        )
        .bind(&month_days)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok::<_, sqlx::Error>(TrafficDailyPoint {
                day: row.try_get("day")?,
                target_code: row.try_get("target_code")?,
                request_count: as_u64(row.try_get("request_count")?),
                response_bytes: as_u64(row.try_get("response_bytes")?),
                error_count: as_u64(row.try_get("error_count")?),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
        let targets = sqlx::query(
            "SELECT target_code, SUM(request_count) AS request_count, SUM(response_bytes) AS response_bytes, SUM(error_count) AS error_count FROM traffic_daily WHERE day LIKE ? GROUP BY target_code ORDER BY response_bytes DESC, request_count DESC LIMIT 10",
        )
        .bind(&month_days)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok::<_, sqlx::Error>(TrafficTargetPoint {
                target_code: row.try_get("target_code")?,
                request_count: as_u64(row.try_get("request_count")?),
                response_bytes: as_u64(row.try_get("response_bytes")?),
                error_count: as_u64(row.try_get("error_count")?),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
        Ok(TrafficOverview {
            request_count: as_u64(request_count),
            response_bytes: as_u64(response_bytes),
            error_count: as_u64(error_count),
            quota_exceeded: quota_exceeded != 0,
            daily,
            targets,
        })
    }

    pub async fn user_usage_overview(
        &self,
        user_id: i64,
        day: &str,
        month: &str,
        default_user_limit: Option<u64>,
    ) -> anyhow::Result<Option<UserUsageOverview>> {
        let Some(profile) = self.user_billing_profile(user_id).await? else {
            return Ok(None);
        };
        let monthly = sqlx::query("SELECT request_count, response_bytes, error_count FROM user_traffic_monthly WHERE month = ? AND user_id = ?")
            .bind(month).bind(user_id).fetch_optional(&self.pool).await?;
        let (request_count, response_bytes, error_count) = monthly
            .map(|row| {
                Ok::<_, sqlx::Error>((
                    row.try_get::<i64, _>("request_count")?,
                    row.try_get::<i64, _>("response_bytes")?,
                    row.try_get::<i64, _>("error_count")?,
                ))
            })
            .transpose()?
            .unwrap_or((0, 0, 0));
        let today = sqlx::query("SELECT COALESCE(SUM(response_bytes), 0) AS bytes FROM user_traffic_daily WHERE day = ? AND user_id = ?")
            .bind(day).bind(user_id).fetch_one(&self.pool).await?.try_get::<i64, _>("bytes")?;
        let month_pattern = format!("{month}-%");
        let daily = sqlx::query("SELECT day, target_code, request_count, response_bytes, error_count FROM user_traffic_daily WHERE user_id = ? AND day LIKE ? ORDER BY day, target_code")
            .bind(user_id).bind(&month_pattern).fetch_all(&self.pool).await?.into_iter().map(|row| Ok::<_, sqlx::Error>(TrafficDailyPoint { day: row.try_get("day")?, target_code: row.try_get("target_code")?, request_count: as_u64(row.try_get("request_count")?), response_bytes: as_u64(row.try_get("response_bytes")?), error_count: as_u64(row.try_get("error_count")?) })).collect::<Result<Vec<_>, _>>()?;
        let targets = sqlx::query("SELECT target_code, SUM(request_count) AS request_count, SUM(response_bytes) AS response_bytes, SUM(error_count) AS error_count FROM user_traffic_daily WHERE user_id = ? AND day LIKE ? GROUP BY target_code ORDER BY response_bytes DESC, request_count DESC")
            .bind(user_id).bind(&month_pattern).fetch_all(&self.pool).await?.into_iter().map(|row| Ok::<_, sqlx::Error>(TrafficTargetPoint { target_code: row.try_get("target_code")?, request_count: as_u64(row.try_get("request_count")?), response_bytes: as_u64(row.try_get("response_bytes")?), error_count: as_u64(row.try_get("error_count")?) })).collect::<Result<Vec<_>, _>>()?;
        let user_limit = match profile.quota_mode.as_str() {
            "unlimited" => None,
            "custom" => profile.user_monthly_limit_bytes,
            _ => default_user_limit,
        };
        let used = as_u64(response_bytes);
        let quota = quota_usage(user_limit, used);
        let group = if let (Some(id), Some(name)) = (profile.group_id, profile.group_name) {
            let used = sqlx::query(
                "SELECT response_bytes FROM group_traffic_monthly WHERE month = ? AND group_id = ?",
            )
            .bind(month)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.try_get::<i64, _>("response_bytes"))
            .transpose()?
            .map(as_u64)
            .unwrap_or(0);
            Some(GroupUsage {
                id,
                name,
                quota: quota_usage(profile.group_monthly_limit_bytes, used),
            })
        } else {
            None
        };
        Ok(Some(UserUsageOverview {
            month: month.to_string(),
            today_response_bytes: as_u64(today),
            request_count: as_u64(request_count),
            response_bytes: used,
            error_count: as_u64(error_count),
            quota,
            group,
            daily,
            targets,
        }))
    }

    pub async fn recent_audit_log(&self, limit: u32) -> anyhow::Result<Vec<AuditLogEntry>> {
        let limit = i64::from(limit.clamp(1, 100));
        sqlx::query(
            "SELECT created_at, username, action, detail FROM config_audit_log ORDER BY created_at DESC, id DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok::<_, sqlx::Error>(AuditLogEntry {
                created_at: row.try_get("created_at")?,
                username: row.try_get("username")?,
                action: row.try_get("action")?,
                detail: row.try_get("detail")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
    }
}

fn as_u64(value: i64) -> u64 {
    value.max(0) as u64
}

fn limit_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn quota_usage(limit_bytes: Option<u64>, used_bytes: u64) -> QuotaUsage {
    QuotaUsage {
        limit_bytes,
        used_bytes,
        remaining_bytes: limit_bytes.map(|limit| limit.saturating_sub(used_bytes)),
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

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn auth_provider_from_row(row: SqliteRow) -> anyhow::Result<AuthProvider> {
    let scopes_json: String = row.try_get("scopes_json")?;
    Ok(AuthProvider {
        id: row.try_get("id")?,
        slug: row.try_get("slug")?,
        display_name: row.try_get("display_name")?,
        kind: row.try_get("kind")?,
        preset: row.try_get("preset")?,
        enabled: row.try_get("enabled")?,
        client_id: row.try_get("client_id")?,
        encrypted_client_secret: row.try_get("encrypted_client_secret")?,
        issuer_url: row.try_get("issuer_url")?,
        authorization_url: row.try_get("authorization_url")?,
        token_url: row.try_get("token_url")?,
        userinfo_url: row.try_get("userinfo_url")?,
        emails_url: row.try_get("emails_url")?,
        scopes: serde_json::from_str(&scopes_json).context("stored provider scopes are invalid")?,
        subject_field: row.try_get("subject_field")?,
        email_field: row.try_get("email_field")?,
        email_verified_field: row.try_get("email_verified_field")?,
        display_name_field: row.try_get("display_name_field")?,
        allow_registration: row.try_get("allow_registration")?,
        auto_link_by_email: row.try_get("auto_link_by_email")?,
    })
}

fn user_account_from_row(row: SqliteRow) -> Result<UserAccount, sqlx::Error> {
    Ok(UserAccount {
        id: row.try_get("id")?,
        email: row.try_get("email")?,
        display_name: row.try_get("display_name")?,
        disabled: row.try_get("disabled")?,
        routing_id: row.try_get("routing_id")?,
        routing_rotated_at: row.try_get("routing_rotated_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

async fn insert_unique_routing_id(
    transaction: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    minimum_length: u8,
    now: i64,
) -> anyhow::Result<String> {
    let sqids = Sqids::builder()
        .alphabet("abcdefghijklmnopqrstuvwxyz0123456789".chars().collect())
        .min_length(minimum_length)
        .build()?;
    for _ in 0..ROUTING_ID_INSERT_ATTEMPTS {
        let public_number = (RandomOsRng.next_u64() & i64::MAX as u64).max(1);
        let routing_id = sqids.encode(&[public_number])?;
        if is_reserved_routing_id(&routing_id) {
            continue;
        }
        let insert = sqlx::query(
            "INSERT OR IGNORE INTO user_routing_ids (user_id, public_number, routing_id, active, created_at) VALUES (?, ?, ?, 1, ?)",
        )
        .bind(user_id)
        .bind(public_number as i64)
        .bind(&routing_id)
        .bind(now)
        .execute(&mut **transaction)
        .await?;
        if insert.rows_affected() == 1 {
            return Ok(routing_id);
        }
    }
    anyhow::bail!("failed to allocate a unique user routing ID")
}

fn is_reserved_routing_id(value: &str) -> bool {
    matches!(
        value,
        "www" | "admin" | "api" | "login" | "account" | "mail" | "smtp" | "status"
    )
}

fn initial_admin_password(configured_password: Option<String>) -> (String, bool) {
    match configured_password.filter(|password| !password.is_empty()) {
        Some(password) => (password, false),
        None => (random_secret(28), true),
    }
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
    "CREATE TABLE IF NOT EXISTS admin_users (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL, role TEXT NOT NULL DEFAULT 'super_admin', disabled INTEGER NOT NULL DEFAULT 0, failed_login_count INTEGER NOT NULL DEFAULT 0, locked_until INTEGER, user_handle TEXT UNIQUE, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS admin_sessions (token_hash TEXT PRIMARY KEY, username TEXT NOT NULL, auth_method TEXT NOT NULL DEFAULT 'password', created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, last_used_at INTEGER NOT NULL, verified_at INTEGER NOT NULL DEFAULT 0, FOREIGN KEY(username) REFERENCES admin_users(username) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS admin_passkeys (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT NOT NULL, name TEXT NOT NULL, credential_id TEXT NOT NULL UNIQUE, passkey_json TEXT NOT NULL, created_at INTEGER NOT NULL, last_used_at INTEGER, FOREIGN KEY(username) REFERENCES admin_users(username) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS admin_webauthn_challenges (challenge_hash TEXT PRIMARY KEY, username TEXT NOT NULL, kind TEXT NOT NULL, state_json TEXT NOT NULL, session_token_hash TEXT, created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, FOREIGN KEY(username) REFERENCES admin_users(username) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL, disabled INTEGER NOT NULL DEFAULT 0, deleted_at INTEGER, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS user_identities (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL, provider_id TEXT NOT NULL, provider_subject TEXT NOT NULL, email TEXT, email_verified INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE(provider_id, provider_subject), FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS user_sessions (token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL, auth_method TEXT NOT NULL, created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, last_used_at INTEGER NOT NULL, FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS groups (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, kind TEXT NOT NULL DEFAULT 'billing', created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS group_members (group_id INTEGER NOT NULL, user_id INTEGER NOT NULL, is_billing INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, PRIMARY KEY(group_id, user_id), FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE, FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS group_quota_settings (group_id INTEGER PRIMARY KEY, monthly_limit_bytes INTEGER, updated_at INTEGER NOT NULL, FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS user_quota_overrides (user_id INTEGER PRIMARY KEY, mode TEXT NOT NULL, monthly_limit_bytes INTEGER, updated_at INTEGER NOT NULL, FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS user_traffic_daily (day TEXT NOT NULL, user_id INTEGER NOT NULL, target_code TEXT NOT NULL, request_count INTEGER NOT NULL DEFAULT 0, response_bytes INTEGER NOT NULL DEFAULT 0, error_count INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(day, user_id, target_code), FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS user_traffic_monthly (month TEXT NOT NULL, user_id INTEGER NOT NULL, request_count INTEGER NOT NULL DEFAULT 0, response_bytes INTEGER NOT NULL DEFAULT 0, error_count INTEGER NOT NULL DEFAULT 0, reserved_bytes INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(month, user_id), FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS group_traffic_monthly (month TEXT NOT NULL, group_id INTEGER NOT NULL, request_count INTEGER NOT NULL DEFAULT 0, response_bytes INTEGER NOT NULL DEFAULT 0, error_count INTEGER NOT NULL DEFAULT 0, reserved_bytes INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(month, group_id), FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS user_routing_ids (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL, public_number INTEGER NOT NULL UNIQUE, routing_id TEXT NOT NULL UNIQUE COLLATE NOCASE, active INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL, revoked_at INTEGER, FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS smtp_settings (singleton INTEGER PRIMARY KEY CHECK(singleton = 1), enabled INTEGER NOT NULL DEFAULT 0, host TEXT NOT NULL DEFAULT '', port INTEGER NOT NULL DEFAULT 587, security TEXT NOT NULL DEFAULT 'starttls', username TEXT, encrypted_password TEXT, from_name TEXT NOT NULL DEFAULT 'MirrorProxy', from_address TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS email_invitations (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT NOT NULL, display_name TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE, status TEXT NOT NULL DEFAULT 'pending', expires_at INTEGER NOT NULL, created_at INTEGER NOT NULL, accepted_at INTEGER, revoked_at INTEGER)",
    "CREATE TABLE IF NOT EXISTS email_login_tokens (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE, code_hash TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, invitation_id INTEGER, expires_at INTEGER NOT NULL, used_at INTEGER, created_at INTEGER NOT NULL, FOREIGN KEY(invitation_id) REFERENCES email_invitations(id) ON DELETE SET NULL)",
    "CREATE TABLE IF NOT EXISTS email_outbox (id INTEGER PRIMARY KEY AUTOINCREMENT, recipient TEXT NOT NULL, subject TEXT NOT NULL, encrypted_body TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending', attempts INTEGER NOT NULL DEFAULT 0, next_attempt_at INTEGER NOT NULL, last_error TEXT, created_at INTEGER NOT NULL, sent_at INTEGER)",
    "CREATE TABLE IF NOT EXISTS email_rate_limits (limit_key TEXT PRIMARY KEY, window_start INTEGER NOT NULL, request_count INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS auth_providers (id INTEGER PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL UNIQUE COLLATE NOCASE, display_name TEXT NOT NULL, kind TEXT NOT NULL, preset TEXT NOT NULL DEFAULT 'custom', enabled INTEGER NOT NULL DEFAULT 0, client_id TEXT NOT NULL, encrypted_client_secret TEXT, issuer_url TEXT, authorization_url TEXT, token_url TEXT, userinfo_url TEXT, emails_url TEXT, scopes_json TEXT NOT NULL, subject_field TEXT NOT NULL DEFAULT 'id', email_field TEXT NOT NULL DEFAULT 'email', email_verified_field TEXT, display_name_field TEXT NOT NULL DEFAULT 'name', allow_registration INTEGER NOT NULL DEFAULT 0, auto_link_by_email INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS user_auth_flows (state_hash TEXT PRIMARY KEY, provider_id INTEGER NOT NULL, encrypted_payload TEXT NOT NULL, mode TEXT NOT NULL, user_id INTEGER, expires_at INTEGER NOT NULL, used_at INTEGER, created_at INTEGER NOT NULL, FOREIGN KEY(provider_id) REFERENCES auth_providers(id) ON DELETE CASCADE, FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE)",
    "CREATE TABLE IF NOT EXISTS proxy_targets (code TEXT PRIMARY KEY, enabled INTEGER NOT NULL, upstream_url TEXT NOT NULL, route_prefix TEXT NOT NULL, updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS traffic_daily (day TEXT NOT NULL, target_code TEXT NOT NULL, request_count INTEGER NOT NULL DEFAULT 0, response_bytes INTEGER NOT NULL DEFAULT 0, upstream_bytes INTEGER NOT NULL DEFAULT 0, error_count INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(day, target_code))",
    "CREATE TABLE IF NOT EXISTS traffic_monthly (month TEXT PRIMARY KEY, request_count INTEGER NOT NULL DEFAULT 0, response_bytes INTEGER NOT NULL DEFAULT 0, upstream_bytes INTEGER NOT NULL DEFAULT 0, error_count INTEGER NOT NULL DEFAULT 0, quota_exceeded INTEGER NOT NULL DEFAULT 0)",
    "CREATE TABLE IF NOT EXISTS request_events (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at INTEGER NOT NULL, target_code TEXT, method TEXT NOT NULL, path TEXT NOT NULL, status_code INTEGER NOT NULL, response_bytes INTEGER NOT NULL DEFAULT 0)",
    "CREATE TABLE IF NOT EXISTS config_audit_log (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at INTEGER NOT NULL, username TEXT NOT NULL, action TEXT NOT NULL, detail TEXT NOT NULL)",
    "CREATE INDEX IF NOT EXISTS admin_sessions_expires_at_idx ON admin_sessions(expires_at)",
    "CREATE INDEX IF NOT EXISTS admin_passkeys_username_idx ON admin_passkeys(username)",
    "CREATE INDEX IF NOT EXISTS admin_webauthn_challenges_expires_at_idx ON admin_webauthn_challenges(expires_at)",
    "CREATE INDEX IF NOT EXISTS user_sessions_expires_at_idx ON user_sessions(expires_at)",
    "CREATE UNIQUE INDEX IF NOT EXISTS user_routing_ids_active_user_idx ON user_routing_ids(user_id) WHERE active = 1",
    "CREATE UNIQUE INDEX IF NOT EXISTS group_members_billing_user_idx ON group_members(user_id) WHERE is_billing = 1",
    "CREATE INDEX IF NOT EXISTS user_traffic_daily_user_idx ON user_traffic_daily(user_id, day)",
    "CREATE INDEX IF NOT EXISTS email_invitations_email_idx ON email_invitations(email, status)",
    "CREATE INDEX IF NOT EXISTS email_login_tokens_email_idx ON email_login_tokens(email, expires_at)",
    "CREATE INDEX IF NOT EXISTS email_outbox_pending_idx ON email_outbox(status, next_attempt_at)",
    "CREATE INDEX IF NOT EXISTS user_auth_flows_expires_idx ON user_auth_flows(expires_at)",
    "CREATE INDEX IF NOT EXISTS request_events_created_at_idx ON request_events(created_at)",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_empty_initial_admin_password_generates_a_random_password() {
        for configured_password in [None, Some(String::new())] {
            let (password, generated) = initial_admin_password(configured_password);
            assert!(generated);
            assert_eq!(password.len(), 28);
            assert!(password
                .chars()
                .all(|character| character.is_ascii_alphanumeric()));
        }
    }

    #[test]
    fn configured_initial_admin_password_is_used_without_policy_validation() {
        let (password, generated) = initial_admin_password(Some("x".to_string()));
        assert!(!generated);
        assert_eq!(password, "x");
    }

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
    async fn changing_password_revokes_existing_sessions() {
        let (database, credentials) = Database::open(":memory:").await.unwrap();
        let credentials = credentials.unwrap();
        let session = database
            .login("admin", &credentials.password)
            .await
            .unwrap()
            .unwrap();
        assert!(!database
            .change_admin_password("admin", "wrong", "next-password-123")
            .await
            .unwrap());
        assert!(database
            .change_admin_password("admin", &credentials.password, "next-password-123")
            .await
            .unwrap());
        assert!(!database.authorize(&session.token).await.unwrap());
        assert!(database
            .login("admin", "next-password-123")
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn locks_an_administrator_after_repeated_failures() {
        let (database, credentials) = Database::open(":memory:").await.unwrap();
        let credentials = credentials.unwrap();
        for attempt in 0..ADMIN_LOGIN_FAILURE_LIMIT {
            let outcome = database
                .login_with_context("admin", "wrong-password", "192.0.2.1")
                .await
                .unwrap();
            if attempt + 1 < ADMIN_LOGIN_FAILURE_LIMIT {
                assert!(matches!(outcome, AdminLoginOutcome::Invalid));
            } else {
                assert!(matches!(outcome, AdminLoginOutcome::Locked { .. }));
            }
        }
        assert!(matches!(
            database
                .login_with_context("admin", &credentials.password, "192.0.2.1")
                .await
                .unwrap(),
            AdminLoginOutcome::Locked { .. }
        ));
    }

    #[tokio::test]
    async fn manages_multiple_admins_and_protects_the_last_super_admin() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        assert!(database
            .create_admin("admin", "operator", "operator-password-123", "admin")
            .await
            .unwrap());
        assert!(!database
            .create_admin("admin", "operator", "another-password-123", "admin")
            .await
            .unwrap());
        assert!(!database
            .set_admin_disabled("admin", "admin", true)
            .await
            .unwrap());
        assert!(database
            .create_admin("admin", "recovery", "recovery-password-123", "super_admin",)
            .await
            .unwrap());
        assert!(database
            .set_admin_disabled("admin", "recovery", true)
            .await
            .unwrap());
        assert!(database
            .login("recovery", "recovery-password-123")
            .await
            .unwrap()
            .is_none());
        let admins = database.list_admins().await.unwrap();
        assert_eq!(admins.len(), 3);
        assert!(admins
            .iter()
            .any(|account| account.username == "operator" && account.role == "admin"));
    }

    #[tokio::test]
    async fn creates_rotates_and_disables_sqids_user_routes() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let user = database
            .create_user("admin", " Person@Example.COM ", "Person", 12)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.email, "person@example.com");
        assert!(user.routing_id.len() >= 12);
        assert!(user
            .routing_id
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit()));
        assert_eq!(
            database
                .user_by_routing_id(&user.routing_id.to_ascii_uppercase())
                .await
                .unwrap()
                .unwrap()
                .user_id,
            user.id
        );
        assert!(matches!(
            database
                .rotate_user_routing_id("user:1", user.id, 12, 24, false)
                .await
                .unwrap(),
            RoutingRotationOutcome::Cooldown { .. }
        ));
        let next = database
            .rotate_user_routing_id("admin", user.id, 12, 24, true)
            .await
            .unwrap();
        let RoutingRotationOutcome::Rotated { routing_id } = next else {
            panic!("administrator rotation should succeed");
        };
        assert_ne!(routing_id, user.routing_id);
        assert!(database
            .user_by_routing_id(&user.routing_id)
            .await
            .unwrap()
            .is_none());
        assert!(database
            .set_user_disabled("admin", user.id, true)
            .await
            .unwrap());
        assert!(database
            .user_by_routing_id(&routing_id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn user_email_and_active_route_are_unique() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let first = database
            .create_user("admin", "person@example.com", "Person", 12)
            .await
            .unwrap()
            .unwrap();
        assert!(database
            .create_user("admin", "PERSON@example.com", "Duplicate", 12)
            .await
            .unwrap()
            .is_none());
        let users = database.list_users().await.unwrap();
        assert_eq!(users, [first]);
    }

    #[tokio::test]
    async fn email_credentials_and_invitations_are_hashed_and_one_time() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let invitation_id = database
            .create_email_invitation(
                "admin",
                "person@example.com",
                "Person",
                "raw-invitation-token",
                unix_timestamp() + 600,
            )
            .await
            .unwrap();
        assert_eq!(
            database
                .valid_invitation("person@example.com", "raw-invitation-token")
                .await
                .unwrap(),
            Some(invitation_id)
        );
        let stored: String = sqlx::query("SELECT token_hash FROM email_invitations WHERE id = ?")
            .bind(invitation_id)
            .fetch_one(&database.pool)
            .await
            .unwrap()
            .try_get("token_hash")
            .unwrap();
        assert_ne!(stored, "raw-invitation-token");

        database
            .store_email_login_token(
                "person@example.com",
                "raw-magic-token",
                "123456",
                Some(invitation_id),
                unix_timestamp() + 600,
            )
            .await
            .unwrap();
        assert_eq!(
            database
                .consume_email_login_token("person@example.com", "raw-magic-token", false)
                .await
                .unwrap(),
            Some(Some(invitation_id))
        );
        assert!(database
            .consume_email_login_token("person@example.com", "raw-magic-token", false)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn email_codes_lock_after_five_failures_and_sends_are_rate_limited() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        database
            .store_email_login_token(
                "person@example.com",
                "token",
                "123456",
                None,
                unix_timestamp() + 600,
            )
            .await
            .unwrap();
        for _ in 0..5 {
            assert!(database
                .consume_email_login_token("person@example.com", "000000", true)
                .await
                .unwrap()
                .is_none());
        }
        assert!(database
            .consume_email_login_token("person@example.com", "123456", true)
            .await
            .unwrap()
            .is_none());

        for _ in 0..3 {
            assert!(database
                .allow_email_send("person@example.com", "192.0.2.1")
                .await
                .unwrap());
        }
        assert!(!database
            .allow_email_send("person@example.com", "192.0.2.1")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn email_outbox_retries_are_bounded_and_errors_are_sanitized() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let id = database
            .enqueue_email("PERSON@example.com", "Subject", "encrypted-value")
            .await
            .unwrap();
        let queued = database.pending_outbox(10).await.unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].recipient, "person@example.com");
        assert_eq!(queued[0].encrypted_body, "encrypted-value");

        database
            .mark_outbox_failed(id, 5, &"x".repeat(300))
            .await
            .unwrap();
        assert!(database.pending_outbox(10).await.unwrap().is_empty());
        let row = sqlx::query(
            "SELECT status, attempts, length(last_error) AS error_length FROM email_outbox WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&database.pool)
        .await
        .unwrap();
        assert_eq!(row.try_get::<String, _>("status").unwrap(), "failed");
        assert_eq!(row.try_get::<i64, _>("attempts").unwrap(), 5);
        assert_eq!(row.try_get::<i64, _>("error_length").unwrap(), 200);
    }

    #[tokio::test]
    async fn user_sessions_are_independent_and_revoked_when_user_is_disabled() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let user = database
            .create_user("email", "person@example.com", "Person", 12)
            .await
            .unwrap()
            .unwrap();
        let session = database
            .create_user_session(user.id, "email")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            database
                .authenticate_user_session(&session.token)
                .await
                .unwrap()
                .unwrap()
                .user_id,
            user.id
        );
        assert!(database
            .authenticate_session(&session.token)
            .await
            .unwrap()
            .is_none());
        database
            .set_user_disabled("admin", user.id, true)
            .await
            .unwrap();
        assert!(database
            .authenticate_user_session(&session.token)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn super_admin_password_reset_revokes_target_sessions() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        database
            .create_admin("admin", "operator", "operator-password-123", "admin")
            .await
            .unwrap();
        let session = database
            .login("operator", "operator-password-123")
            .await
            .unwrap()
            .unwrap();
        assert!(database
            .reset_admin_password("admin", "operator", "replacement-password-123")
            .await
            .unwrap());
        assert!(!database.authorize(&session.token).await.unwrap());
        assert!(database
            .login("operator", "replacement-password-123")
            .await
            .unwrap()
            .is_some());
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
                reserved_bytes: 0,
                user_id: None,
                group_id: None,
                request_event_retention_days: 30,
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
                reserved_bytes: 0,
                user_id: None,
                group_id: None,
                request_event_retention_days: 30,
            })
            .await
            .unwrap();

        database.mark_month_quota_exceeded("2026-07").await.unwrap();
        let overview = database.traffic_overview("2026-07").await.unwrap();
        assert_eq!(overview.request_count, 2);
        assert_eq!(overview.response_bytes, 1036);
        assert_eq!(overview.error_count, 1);
        assert!(overview.quota_exceeded);
        assert_eq!(overview.targets[0].target_code, "npm");
    }

    #[tokio::test]
    async fn prunes_expired_request_events_when_recording_traffic() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        sqlx::query("INSERT INTO request_events (created_at, method, path, status_code, response_bytes) VALUES (?, 'GET', '/old', 200, 1)")
            .bind(unix_timestamp() - 2 * 24 * 60 * 60)
            .execute(&database.pool)
            .await
            .unwrap();
        database
            .record_proxy_response(ProxyTrafficRecord {
                day: "2026-07-10",
                month: "2026-07",
                target_code: "npm",
                method: "GET",
                path: "/npm/react",
                status_code: 200,
                response_bytes: 1,
                stream_error: false,
                reserved_bytes: 0,
                user_id: None,
                group_id: None,
                request_event_retention_days: 1,
            })
            .await
            .unwrap();
        let count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM request_events")
            .fetch_one(&database.pool)
            .await
            .unwrap()
            .try_get("count")
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn reserves_monthly_capacity_atomically() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        assert!(database
            .try_reserve_monthly_bytes("2026-07", 10, 6)
            .await
            .unwrap());
        assert!(!database
            .try_reserve_monthly_bytes("2026-07", 10, 6)
            .await
            .unwrap());
        database
            .record_proxy_response(ProxyTrafficRecord {
                day: "2026-07-01",
                month: "2026-07",
                target_code: "npm",
                method: "GET",
                path: "/npm/pkg",
                status_code: 200,
                response_bytes: 4,
                stream_error: false,
                reserved_bytes: 6,
                user_id: None,
                group_id: None,
                request_event_retention_days: 30,
            })
            .await
            .unwrap();
        assert!(database
            .try_reserve_monthly_bytes("2026-07", 10, 6)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn hierarchical_quota_reservations_are_atomic_and_usage_is_attributed() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let first = database
            .create_user("admin", "first@example.com", "First", 12)
            .await
            .unwrap()
            .unwrap();
        let second = database
            .create_user("admin", "second@example.com", "Second", 12)
            .await
            .unwrap()
            .unwrap();
        let group = database
            .create_billing_group("admin", "Engineering", Some(10))
            .await
            .unwrap()
            .unwrap();
        assert!(database
            .set_user_billing_profile("admin", first.id, Some(group.id), "custom", Some(8))
            .await
            .unwrap());
        assert!(database
            .set_user_billing_profile("admin", second.id, Some(group.id), "unlimited", None)
            .await
            .unwrap());

        assert_eq!(
            database
                .try_reserve_hierarchical_bytes("2026-07", first.id, Some(20), None, 6)
                .await
                .unwrap(),
            HierarchicalReservationOutcome::Reserved {
                group_id: Some(group.id)
            }
        );
        assert_eq!(
            database
                .try_reserve_hierarchical_bytes("2026-07", first.id, Some(20), None, 3)
                .await
                .unwrap(),
            HierarchicalReservationOutcome::Exceeded { scope: "user" }
        );
        database
            .record_proxy_response(ProxyTrafficRecord {
                day: "2026-07-10",
                month: "2026-07",
                target_code: "npm",
                method: "GET",
                path: "/npm/react",
                status_code: 200,
                response_bytes: 4,
                stream_error: false,
                reserved_bytes: 6,
                user_id: Some(first.id),
                group_id: Some(group.id),
                request_event_retention_days: 30,
            })
            .await
            .unwrap();
        assert!(matches!(
            database
                .try_reserve_hierarchical_bytes("2026-07", first.id, Some(20), None, 4)
                .await
                .unwrap(),
            HierarchicalReservationOutcome::Reserved { .. }
        ));
        assert_eq!(
            database
                .try_reserve_hierarchical_bytes("2026-07", second.id, Some(20), None, 3)
                .await
                .unwrap(),
            HierarchicalReservationOutcome::Exceeded { scope: "group" }
        );
        database
            .record_proxy_response(ProxyTrafficRecord {
                day: "2026-07-10",
                month: "2026-07",
                target_code: "npm",
                method: "GET",
                path: "/npm/vue",
                status_code: 200,
                response_bytes: 4,
                stream_error: false,
                reserved_bytes: 4,
                user_id: Some(first.id),
                group_id: Some(group.id),
                request_event_retention_days: 30,
            })
            .await
            .unwrap();
        let usage = database
            .user_usage_overview(first.id, "2026-07-10", "2026-07", None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(usage.today_response_bytes, 8);
        assert_eq!(usage.quota.used_bytes, 8);
        assert_eq!(usage.quota.remaining_bytes, Some(0));
        assert_eq!(usage.group.unwrap().quota.used_bytes, 8);
    }

    #[tokio::test]
    async fn returns_recent_audit_log() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        database
            .save_runtime_config("admin", &Config::default(), "update runtime configuration")
            .await
            .unwrap();

        let entries = database.recent_audit_log(10).await.unwrap();
        assert!(entries.iter().any(|entry| entry.username == "admin"
            && entry.action == "update runtime configuration"
            && entry.detail == "runtime_config"));
    }

    #[tokio::test]
    async fn stores_provider_secrets_and_preserves_them_on_update() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let mut provider = test_auth_provider("github", "encrypted-secret");
        let id = database
            .save_auth_provider("admin", &provider, false)
            .await
            .unwrap();
        provider.id = id;
        provider.display_name = "GitHub Enterprise".to_string();
        provider.encrypted_client_secret = None;
        database
            .save_auth_provider("admin", &provider, true)
            .await
            .unwrap();

        let stored = database.auth_provider_by_id(id).await.unwrap().unwrap();
        assert_eq!(stored.display_name, "GitHub Enterprise");
        assert_eq!(
            stored.encrypted_client_secret.as_deref(),
            Some("encrypted-secret")
        );
        let serialized = serde_json::to_string(&stored).unwrap();
        assert!(!serialized.contains("encrypted-secret"));
    }

    #[tokio::test]
    async fn authentication_flows_are_hashed_and_consumed_once() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let provider_id = database
            .save_auth_provider("admin", &test_auth_provider("github", "secret"), false)
            .await
            .unwrap();
        database
            .store_user_auth_flow(
                "raw-state",
                provider_id,
                "encrypted-flow",
                "login",
                None,
                unix_timestamp() + 60,
            )
            .await
            .unwrap();
        let stored_state: String = sqlx::query_scalar("SELECT state_hash FROM user_auth_flows")
            .fetch_one(&database.pool)
            .await
            .unwrap();
        assert_ne!(stored_state, "raw-state");
        assert_eq!(
            database
                .take_user_auth_flow("raw-state")
                .await
                .unwrap()
                .unwrap()
                .encrypted_payload,
            "encrypted-flow"
        );
        assert!(database
            .take_user_auth_flow("raw-state")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn external_identity_is_unique_and_provider_slug_changes_follow_bindings() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let first = database
            .create_user("admin", "first@example.com", "First", 12)
            .await
            .unwrap()
            .unwrap();
        let second = database
            .create_user("admin", "second@example.com", "Second", 12)
            .await
            .unwrap()
            .unwrap();
        let mut provider = test_auth_provider("github", "secret");
        provider.id = database
            .save_auth_provider("admin", &provider, false)
            .await
            .unwrap();
        assert!(database
            .bind_external_identity(
                "user",
                first.id,
                "github",
                "subject-1",
                Some("first@example.com"),
                true,
            )
            .await
            .unwrap());
        assert!(!database
            .bind_external_identity(
                "user",
                second.id,
                "github",
                "subject-1",
                Some("second@example.com"),
                true,
            )
            .await
            .unwrap());
        provider.slug = "company-github".to_string();
        provider.encrypted_client_secret = None;
        database
            .save_auth_provider("admin", &provider, true)
            .await
            .unwrap();
        assert_eq!(
            database
                .user_by_external_identity("company-github", "subject-1")
                .await
                .unwrap()
                .unwrap()
                .id,
            first.id
        );
        assert_eq!(
            database
                .auth_provider_identity_count(provider.id)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn external_registration_is_atomic_and_accepts_the_matching_invitation() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let invitation_id = database
            .create_email_invitation(
                "admin",
                "first@example.com",
                "First",
                "invitation-token",
                unix_timestamp() + 60,
            )
            .await
            .unwrap();
        let first = database
            .create_user_with_external_identity(ExternalRegistration {
                actor: "oauth:github",
                email: "first@example.com",
                display_name: "First",
                routing_min_length: 12,
                provider_slug: "github",
                provider_subject: "subject-1",
                invitation_id: Some(invitation_id),
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            database
                .email_invitation(invitation_id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "accepted"
        );
        assert_eq!(
            database
                .user_by_external_identity("github", "subject-1")
                .await
                .unwrap()
                .unwrap()
                .id,
            first.id
        );

        assert!(database
            .create_user_with_external_identity(ExternalRegistration {
                actor: "oauth:github",
                email: "orphan@example.com",
                display_name: "Orphan",
                routing_min_length: 12,
                provider_slug: "github",
                provider_subject: "subject-1",
                invitation_id: None,
            },)
            .await
            .is_err());
        assert!(database
            .user_by_email("orphan@example.com")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn lists_and_revokes_individual_administrator_sessions() {
        let (database, credentials) = Database::open(":memory:").await.unwrap();
        let credentials = credentials.unwrap();
        let first = database
            .login("admin", &credentials.password)
            .await
            .unwrap()
            .unwrap();
        let second = database
            .login("admin", &credentials.password)
            .await
            .unwrap()
            .unwrap();
        let sessions = database
            .list_admin_sessions("admin", &first.token)
            .await
            .unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions.iter().filter(|session| session.current).count(), 1);
        let second_id = sessions
            .iter()
            .find(|session| !session.current)
            .unwrap()
            .id
            .clone();
        assert!(database
            .revoke_admin_session("admin", "admin", &second_id)
            .await
            .unwrap());
        assert!(database.authorize(&first.token).await.unwrap());
        assert!(!database.authorize(&second.token).await.unwrap());
    }

    #[tokio::test]
    async fn soft_delete_revokes_user_access_but_preserves_the_record() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let user = database
            .create_user("admin", "delete@example.com", "Delete", 12)
            .await
            .unwrap()
            .unwrap();
        let session = database
            .create_user_session(user.id, "email")
            .await
            .unwrap()
            .unwrap();
        assert!(database.soft_delete_user("admin", user.id).await.unwrap());
        assert!(database
            .authenticate_user_session(&session.token)
            .await
            .unwrap()
            .is_none());
        assert!(database
            .user_by_routing_id(&user.routing_id)
            .await
            .unwrap()
            .is_none());
        let deleted_at: Option<i64> =
            sqlx::query_scalar("SELECT deleted_at FROM users WHERE id = ?")
                .bind(user.id)
                .fetch_one(&database.pool)
                .await
                .unwrap();
        assert!(deleted_at.is_some());
    }

    fn test_auth_provider(slug: &str, encrypted_secret: &str) -> AuthProvider {
        AuthProvider {
            id: 0,
            slug: slug.to_string(),
            display_name: "GitHub".to_string(),
            kind: "oauth2".to_string(),
            preset: "github".to_string(),
            enabled: true,
            client_id: "client-id".to_string(),
            encrypted_client_secret: Some(encrypted_secret.to_string()),
            issuer_url: None,
            authorization_url: Some("https://github.com/login/oauth/authorize".to_string()),
            token_url: Some("https://github.com/login/oauth/access_token".to_string()),
            userinfo_url: Some("https://api.github.com/user".to_string()),
            emails_url: Some("https://api.github.com/user/emails".to_string()),
            scopes: vec!["read:user".to_string(), "user:email".to_string()],
            subject_field: "id".to_string(),
            email_field: "email".to_string(),
            email_verified_field: None,
            display_name_field: "name".to_string(),
            allow_registration: true,
            auto_link_by_email: false,
        }
    }
}
