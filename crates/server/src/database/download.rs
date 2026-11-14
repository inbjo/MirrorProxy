//! Download permissions, hashed scoped tokens, domain policy and bounded daily aggregates.
use super::*;
use serde_json::{json, Value};

impl Database {
    pub(super) async fn migrate_downloads(&self) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        for statement in [
            "CREATE TABLE IF NOT EXISTS download_permissions (kind TEXT NOT NULL CHECK(kind IN ('user','group')), subject_id INTEGER NOT NULL, policy TEXT NOT NULL CHECK(policy IN ('allow','deny')), PRIMARY KEY(kind,subject_id))",
            "CREATE TABLE IF NOT EXISTS download_tokens (id TEXT PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id), token_hash TEXT NOT NULL UNIQUE, name TEXT NOT NULL, expires_at INTEGER NOT NULL, created_at INTEGER NOT NULL)",
            "CREATE INDEX IF NOT EXISTS download_tokens_user ON download_tokens(user_id)",
            "CREATE TABLE IF NOT EXISTS download_blocks (domain TEXT PRIMARY KEY, subdomains INTEGER NOT NULL, reason TEXT NOT NULL, actor TEXT NOT NULL, created_at INTEGER NOT NULL)",
            "CREATE TABLE IF NOT EXISTS download_usage (day TEXT NOT NULL, user_id INTEGER NOT NULL REFERENCES users(id), domain TEXT NOT NULL, entry_domain TEXT NOT NULL, requests INTEGER NOT NULL DEFAULT 0, errors INTEGER NOT NULL DEFAULT 0, denied INTEGER NOT NULL DEFAULT 0, bytes INTEGER NOT NULL DEFAULT 0, upstream_bytes INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(day,user_id,domain,entry_domain))",
            "CREATE INDEX IF NOT EXISTS download_usage_domain ON download_usage(domain,day)",
        ] {
            sqlx::query(statement).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations(version,name,applied_at) VALUES (?,'controlled-downloads',?)").bind(CURRENT_SCHEMA_VERSION).bind(unix_timestamp()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn download_allowed(&self, user: i64) -> anyhow::Result<bool> {
        // Explicit user deny wins. Group membership is evaluated on every request/chunk.
        let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u WHERE u.id=? AND u.disabled=0 AND u.deleted_at IS NULL AND COALESCE((SELECT policy='allow' FROM download_permissions WHERE kind='user' AND subject_id=u.id), EXISTS(SELECT 1 FROM group_members m JOIN download_permissions p ON p.kind='group' AND p.subject_id=m.group_id AND p.policy='allow' WHERE m.user_id=u.id)))")
            .bind(user).fetch_one(&self.pool).await?;
        Ok(allowed)
    }

    pub async fn download_permissions(&self) -> anyhow::Result<Value> {
        let rows = sqlx::query(
            "SELECT p.kind,p.subject_id,p.policy, CASE WHEN p.kind='user' THEN u.display_name ELSE g.name END AS subject_name, u.email AS subject_email, u.username AS subject_username FROM download_permissions p LEFT JOIN users u ON p.kind='user' AND u.id=p.subject_id LEFT JOIN groups g ON p.kind='group' AND g.id=p.subject_id ORDER BY p.kind,p.subject_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(json!(rows
            .into_iter()
            .map(|r| json!({
                "kind": r.get::<String, _>("kind"),
                "subject_id": r.get::<i64, _>("subject_id"),
                "policy": r.get::<String, _>("policy"),
                "subject_name": r.get::<Option<String>, _>("subject_name"),
                "subject_email": r.get::<Option<String>, _>("subject_email"),
                "subject_username": r.get::<Option<String>, _>("subject_username"),
            }))
            .collect::<Vec<_>>()))
    }

    pub async fn set_download_permission(
        &self,
        actor: &str,
        kind: &str,
        id: i64,
        policy: &str,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let exists: bool = if kind == "user" {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM users WHERE id=? AND deleted_at IS NULL)",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM groups WHERE id=?)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?
        };
        if !exists {
            return Ok(false);
        }
        sqlx::query("DELETE FROM download_permissions WHERE kind=? AND subject_id=?")
            .bind(kind)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if policy != "inherit" {
            sqlx::query("INSERT INTO download_permissions(kind,subject_id,policy) VALUES(?,?,?)")
                .bind(kind)
                .bind(id)
                .bind(policy)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("INSERT INTO config_audit_log(created_at,username,action,detail) VALUES(?,?,'download_permission_changed',?)").bind(unix_timestamp()).bind(actor).bind(format!("{kind}:{id} {policy}")).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn create_download_token(
        &self,
        user: i64,
        name: &str,
        days: u32,
    ) -> anyhow::Result<Option<Value>> {
        if !self.download_allowed(user).await? {
            return Ok(None);
        }
        let token = format!("mpd_{}", random_secret(48));
        let id = Uuid::new_v4().to_string();
        let expires = unix_timestamp() + i64::from(days) * 86400;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM download_tokens WHERE expires_at<=?")
            .bind(unix_timestamp())
            .execute(&mut *tx)
            .await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_tokens WHERE user_id=?")
            .bind(user)
            .fetch_one(&mut *tx)
            .await?;
        anyhow::ensure!(count < 20, "at most 20 active download tokens");
        sqlx::query("INSERT INTO download_tokens VALUES(?,?,?,?,?,?)")
            .bind(&id)
            .bind(user)
            .bind(hash_token(&token))
            .bind(name)
            .bind(expires)
            .bind(unix_timestamp())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO config_audit_log(created_at,username,action,detail) VALUES(?,?,'download_token_created',?)").bind(unix_timestamp()).bind(format!("user:{user}")).bind(&id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(
            json!({"id":id,"token":token,"scope":"download:fetch","expires_at":expires}),
        ))
    }

    pub async fn download_tokens(&self, user: i64) -> anyhow::Result<Value> {
        let rows = sqlx::query("SELECT id,name,expires_at,created_at FROM download_tokens WHERE user_id=? AND expires_at>? ORDER BY created_at DESC").bind(user).bind(unix_timestamp()).fetch_all(&self.pool).await?;
        Ok(json!(rows.into_iter().map(|r| json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"expires_at":r.get::<i64,_>("expires_at"),"created_at":r.get::<i64,_>("created_at"),"scope":"download:fetch"})).collect::<Vec<_>>()))
    }

    pub async fn revoke_download_token(&self, user: i64, id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM download_tokens WHERE user_id=? AND id=?")
            .bind(user)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.append_audit_log(&format!("user:{user}"), "download_token_revoked", id)
            .await
    }

    pub async fn authenticate_download_token(&self, token: &str) -> anyhow::Result<Option<i64>> {
        if !token.starts_with("mpd_") || token.len() != 52 {
            return Ok(None);
        }
        let user: Option<i64> = sqlx::query_scalar(
            "SELECT user_id FROM download_tokens WHERE token_hash=? AND expires_at>?",
        )
        .bind(hash_token(token))
        .bind(unix_timestamp())
        .fetch_optional(&self.pool)
        .await?;
        match user {
            Some(id) if self.download_allowed(id).await? => Ok(Some(id)),
            _ => Ok(None),
        }
    }

    pub async fn download_blocked(&self, domain: &str) -> anyhow::Result<bool> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM download_blocks WHERE domain=? OR (subdomains=1 AND substr(?, -length(domain)-1)='.' || domain))").bind(domain).bind(domain).fetch_one(&self.pool).await?)
    }

    pub async fn download_blocks(&self) -> anyhow::Result<Value> {
        let rows = sqlx::query("SELECT * FROM download_blocks ORDER BY domain")
            .fetch_all(&self.pool)
            .await?;
        Ok(json!(rows.into_iter().map(|r| json!({"domain":r.get::<String,_>("domain"),"subdomains":r.get::<bool,_>("subdomains"),"reason":r.get::<String,_>("reason"),"actor":r.get::<String,_>("actor"),"created_at":r.get::<i64,_>("created_at")})).collect::<Vec<_>>()))
    }

    pub async fn set_download_block(
        &self,
        actor: &str,
        domain: &str,
        subdomains: bool,
        reason: &str,
        blocked: bool,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM download_blocks WHERE domain=?")
            .bind(domain)
            .execute(&mut *tx)
            .await?;
        if blocked {
            sqlx::query("INSERT INTO download_blocks VALUES(?,?,?,?,?)")
                .bind(domain)
                .bind(subdomains)
                .bind(reason)
                .bind(actor)
                .bind(unix_timestamp())
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("INSERT INTO config_audit_log(created_at,username,action,detail) VALUES(?,?,'download_domain_policy_changed',?)").bind(unix_timestamp()).bind(actor).bind(format!("{domain} blocked={blocked} subdomains={subdomains} {reason}")).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn download_upstream(
        &self,
        user: i64,
        domain: &str,
        entry: &str,
        bytes: u64,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO download_usage(upstream_bytes,day,user_id,domain,entry_domain) VALUES(?,?,?,?,?) ON CONFLICT(day,user_id,domain,entry_domain) DO UPDATE SET upstream_bytes=upstream_bytes+excluded.upstream_bytes").bind(limit_to_i64(bytes)).bind(UtcDay::today()).bind(user).bind(domain).bind(entry).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn download_stream_error(
        &self,
        user: i64,
        domain: &str,
        entry: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE download_usage SET errors=errors+1 WHERE day=? AND user_id=? AND domain=? AND entry_domain=?").bind(UtcDay::today()).bind(user).bind(domain).bind(entry).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn download_event(
        &self,
        user: i64,
        domain: &str,
        entry: &str,
        error: bool,
        denied: bool,
        config: &Config,
    ) -> anyhow::Result<()> {
        let day = UtcDay::today();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(90))
            .format("%Y-%m-%d")
            .to_string();
        sqlx::query("DELETE FROM download_usage WHERE day<?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO download_usage(day,user_id,domain,entry_domain,requests,errors,denied) VALUES(?,?,?,?,1,?,?) ON CONFLICT(day,user_id,domain,entry_domain) DO UPDATE SET requests=requests+1,errors=errors+excluded.errors,denied=denied+excluded.denied")
            .bind(day).bind(user).bind(domain).bind(entry).bind(error).bind(denied).execute(&mut *tx).await?;
        let (day, month) = crate::quota_period(&config.quota.timezone);
        sqlx::query("INSERT INTO traffic_daily(day,target_code,request_count,error_count) VALUES(?,'download',1,?) ON CONFLICT(day,target_code) DO UPDATE SET request_count=request_count+1,error_count=error_count+excluded.error_count").bind(&day).bind(error).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO traffic_monthly(month,request_count,error_count) VALUES(?,1,?) ON CONFLICT(month) DO UPDATE SET request_count=request_count+1,error_count=error_count+excluded.error_count").bind(&month).bind(error).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO user_traffic_daily(day,user_id,target_code,request_count,error_count) VALUES(?,?,'download',1,?) ON CONFLICT(day,user_id,target_code) DO UPDATE SET request_count=request_count+1,error_count=error_count+excluded.error_count").bind(&day).bind(user).bind(error).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO user_traffic_monthly(month,user_id,request_count,error_count) VALUES(?,?,1,?) ON CONFLICT(month,user_id) DO UPDATE SET request_count=request_count+1,error_count=error_count+excluded.error_count").bind(&month).bind(user).bind(error).execute(&mut *tx).await?;
        let group: Option<i64> = sqlx::query_scalar(
            "SELECT group_id FROM group_members WHERE user_id=? AND is_billing=1",
        )
        .bind(user)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(group) = group {
            sqlx::query("INSERT INTO group_traffic_monthly(month,group_id,request_count,error_count) VALUES(?,?,1,?) ON CONFLICT(month,group_id) DO UPDATE SET request_count=request_count+1,error_count=error_count+excluded.error_count").bind(&month).bind(group).bind(error).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Charge a bounded chunk before handing it to the client. All budgets share
    /// one transaction, so simultaneous downloads cannot overspend a quota.
    pub async fn charge_download(
        &self,
        user: i64,
        domain: &str,
        entry: &str,
        bytes: u64,
        config: &Config,
    ) -> anyhow::Result<bool> {
        let (day, month) = crate::quota_period(&config.quota.timezone);
        let daily_day = UtcDay::today();
        let billed = limit_to_i64(
            bytes
                * if config.quota.bidirectional_accounting {
                    2
                } else {
                    1
                },
        );
        let bytes = limit_to_i64(bytes);
        let mut tx = self.pool.begin().await?;
        // Acquire the writer before reading budgets, avoiding deferred-tx races.
        sqlx::query(
            "INSERT OR IGNORE INTO download_usage(day,user_id,domain,entry_domain) VALUES(?,?,?,?)",
        )
        .bind(&daily_day)
        .bind(user)
        .bind(domain)
        .bind(entry)
        .execute(&mut *tx)
        .await?;
        let used: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(bytes),0) FROM download_usage WHERE day=? AND user_id=?",
        )
        .bind(&daily_day)
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
        if used.saturating_add(bytes) > limit_to_i64(config.download.daily_user_bytes) {
            return Ok(false);
        }
        let profile=sqlx::query("SELECT gm.group_id,gq.monthly_limit_bytes AS group_limit,uq.mode,uq.monthly_limit_bytes AS user_limit FROM users u LEFT JOIN group_members gm ON gm.user_id=u.id AND gm.is_billing=1 LEFT JOIN group_quota_settings gq ON gq.group_id=gm.group_id LEFT JOIN user_quota_overrides uq ON uq.user_id=u.id WHERE u.id=? AND u.disabled=0 AND u.deleted_at IS NULL").bind(user).fetch_optional(&mut *tx).await?;
        let Some(profile) = profile else {
            return Ok(false);
        };
        let group: Option<i64> = profile.try_get("group_id")?;
        let user_limit = match profile.try_get::<Option<String>, _>("mode")?.as_deref() {
            Some("unlimited") => None,
            Some("custom") => profile.try_get::<Option<i64>, _>("user_limit")?,
            _ => config
                .quota
                .default_user_monthly_gb
                .map(|g| limit_to_i64(g.saturating_mul(1 << 30))),
        };
        sqlx::query("INSERT OR IGNORE INTO traffic_monthly(month) VALUES(?)")
            .bind(&month)
            .execute(&mut *tx)
            .await?;
        let limit = if config.quota.enabled {
            limit_to_i64(config.quota.monthly_gb.saturating_mul(1 << 30))
        } else {
            i64::MAX
        };
        if sqlx::query("UPDATE traffic_monthly SET response_bytes=response_bytes+? WHERE month=? AND response_bytes+reserved_bytes+?<=?").bind(billed).bind(&month).bind(billed).bind(limit).execute(&mut *tx).await?.rows_affected()==0 { return Ok(false); }
        sqlx::query("INSERT OR IGNORE INTO user_traffic_monthly(month,user_id) VALUES(?,?)")
            .bind(&month)
            .bind(user)
            .execute(&mut *tx)
            .await?;
        if sqlx::query("UPDATE user_traffic_monthly SET response_bytes=response_bytes+? WHERE month=? AND user_id=? AND response_bytes+reserved_bytes+?<=?").bind(billed).bind(&month).bind(user).bind(billed).bind(user_limit.unwrap_or(i64::MAX)).execute(&mut *tx).await?.rows_affected()==0 { return Ok(false); }
        if let Some(group) = group {
            sqlx::query("INSERT OR IGNORE INTO group_traffic_monthly(month,group_id) VALUES(?,?)")
                .bind(&month)
                .bind(group)
                .execute(&mut *tx)
                .await?;
            if sqlx::query("UPDATE group_traffic_monthly SET response_bytes=response_bytes+? WHERE month=? AND group_id=? AND response_bytes+reserved_bytes+?<=?").bind(billed).bind(&month).bind(group).bind(billed).bind(profile.try_get::<Option<i64>,_>("group_limit")?.unwrap_or(i64::MAX)).execute(&mut *tx).await?.rows_affected()==0 { return Ok(false); }
        }
        sqlx::query("INSERT INTO traffic_daily(day,target_code,response_bytes) VALUES(?,'download',?) ON CONFLICT(day,target_code) DO UPDATE SET response_bytes=response_bytes+excluded.response_bytes").bind(&day).bind(billed).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO user_traffic_daily(day,user_id,target_code,response_bytes) VALUES(?,?,'download',?) ON CONFLICT(day,user_id,target_code) DO UPDATE SET response_bytes=response_bytes+excluded.response_bytes").bind(&day).bind(user).bind(billed).execute(&mut *tx).await?;
        sqlx::query("UPDATE download_usage SET bytes=bytes+? WHERE day=? AND user_id=? AND domain=? AND entry_domain=?").bind(bytes).bind(daily_day).bind(user).bind(domain).bind(entry).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn download_stats(
        &self,
        days: u32,
        user: Option<i64>,
        domain: Option<&str>,
    ) -> anyhow::Result<Value> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(i64::from(days) - 1))
            .format("%Y-%m-%d")
            .to_string();
        let rows=sqlx::query("SELECT day,user_id,domain,entry_domain,SUM(requests) AS requests,SUM(errors) AS errors,SUM(denied) AS denied,SUM(bytes) AS bytes,SUM(upstream_bytes) AS upstream_bytes FROM download_usage WHERE day>=? AND (? IS NULL OR user_id=?) AND (? IS NULL OR domain=? OR entry_domain=?) GROUP BY day,user_id,domain,entry_domain ORDER BY day DESC,bytes DESC LIMIT 2000").bind(cutoff).bind(user).bind(user).bind(domain).bind(domain).bind(domain).fetch_all(&self.pool).await?;
        let truncated = rows.len() == 2000;
        let details=json!(rows.into_iter().map(|r| json!({"day":r.get::<String,_>("day"),"user_id":r.get::<i64,_>("user_id"),"domain":r.get::<String,_>("domain"),"entry_domain":r.get::<String,_>("entry_domain"),"requests":r.get::<i64,_>("requests"),"errors":r.get::<i64,_>("errors"),"denied":r.get::<i64,_>("denied"),"bytes":r.get::<i64,_>("bytes"),"upstream_bytes":r.get::<i64,_>("upstream_bytes")})).collect::<Vec<_>>());
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(i64::from(days) - 1))
            .format("%Y-%m-%d")
            .to_string();
        let mut aggregates = serde_json::Map::new();
        for (key, selection, group, limit) in [
            ("domains", "domain", "GROUP BY domain", 200),
            ("users", "user_id", "GROUP BY user_id", 200),
            ("daily", "day", "GROUP BY day", 90),
            ("totals", "'all' AS scope", "", 1),
        ] {
            let sql=format!("SELECT {selection},COALESCE(SUM(requests),0) AS requests,COALESCE(SUM(errors),0) AS errors,COALESCE(SUM(denied),0) AS denied,COALESCE(SUM(bytes),0) AS bytes,COALESCE(SUM(upstream_bytes),0) AS upstream_bytes FROM download_usage WHERE day>=? AND (? IS NULL OR user_id=?) AND (? IS NULL OR domain=? OR entry_domain=?) {group} ORDER BY bytes DESC LIMIT {limit}");
            // Projection, grouping and limit come from the literal list above;
            // every request-supplied filter remains a bound parameter.
            let data = sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(&cutoff)
                .bind(user)
                .bind(user)
                .bind(domain)
                .bind(domain)
                .bind(domain)
                .fetch_all(&self.pool)
                .await?;
            let values=data.into_iter().map(|r|{
                let mut v=json!({"requests":r.get::<i64,_>("requests"),"errors":r.get::<i64,_>("errors"),"denied":r.get::<i64,_>("denied"),"bytes":r.get::<i64,_>("bytes"),"upstream_bytes":r.get::<i64,_>("upstream_bytes")});
                match key {"domains"=>v["domain"]=json!(r.get::<String,_>("domain")),"users"=>v["user_id"]=json!(r.get::<i64,_>("user_id")),"daily"=>v["day"]=json!(r.get::<String,_>("day")),_=>{}}
                v
            }).collect::<Vec<_>>();
            aggregates.insert(key.into(), json!(values));
        }
        aggregates.insert("rows".into(), details);
        aggregates.insert("truncated".into(), json!(truncated));
        Ok(Value::Object(aggregates))
    }
}
struct UtcDay;
impl UtcDay {
    fn today() -> String {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn fixture() -> (Database, i64) {
        let (db, _) = Database::open(":memory:").await.unwrap();
        let user = db
            .create_user("admin", "artifact@example.org", "Artifact", 12)
            .await
            .unwrap()
            .unwrap();
        (db, user.id)
    }
    #[tokio::test]
    async fn permission_report_resolves_current_user_and_group_identity() {
        let (db, user) = fixture().await;
        let group = db
            .create_billing_group("admin", "Download team", None)
            .await
            .unwrap()
            .unwrap();
        db.set_download_permission("admin", "user", user, "allow")
            .await
            .unwrap();
        db.set_download_permission("admin", "group", group.id, "deny")
            .await
            .unwrap();
        sqlx::query(
            "UPDATE users SET username='artifact-account', display_name='Current name' WHERE id=?",
        )
        .bind(user)
        .execute(&db.pool)
        .await
        .unwrap();
        let report = db.download_permissions().await.unwrap();
        let rows = report.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        let user_row = rows.iter().find(|row| row["kind"] == "user").unwrap();
        assert_eq!(user_row["subject_id"], user);
        assert_eq!(user_row["policy"], "allow");
        assert_eq!(user_row["subject_name"], "Current name");
        assert_eq!(user_row["subject_username"], "artifact-account");
        assert_eq!(user_row["subject_email"], "artifact@example.org");
        let group_row = rows.iter().find(|row| row["kind"] == "group").unwrap();
        assert_eq!(group_row["subject_name"], "Download team");
        assert_eq!(group_row["policy"], "deny");
        assert!(group_row["subject_email"].is_null());
        assert!(group_row["subject_username"].is_null());
    }

    #[tokio::test]
    async fn permissions_tokens_and_revocation_follow_current_user_and_group_state() {
        let (db, user) = fixture().await;
        let group = db
            .create_billing_group("admin", "Downloads", None)
            .await
            .unwrap()
            .unwrap();
        db.set_user_billing_profile("admin", user, Some(group.id), "default", None)
            .await
            .unwrap();
        assert!(!db.download_allowed(user).await.unwrap());
        db.set_download_permission("admin", "group", group.id, "allow")
            .await
            .unwrap();
        assert!(db.download_allowed(user).await.unwrap());
        let data = db
            .create_download_token(user, "Agent", 1)
            .await
            .unwrap()
            .unwrap();
        let token = data["token"].as_str().unwrap();
        let id = data["id"].as_str().unwrap();
        assert_eq!(
            db.authenticate_download_token(token).await.unwrap(),
            Some(user)
        );
        let stored: String = sqlx::query_scalar("SELECT token_hash FROM download_tokens")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_ne!(stored, token);
        assert_eq!(stored, hash_token(token));
        assert!(!db
            .download_tokens(user)
            .await
            .unwrap()
            .to_string()
            .contains(token));
        db.set_download_permission("admin", "user", user, "deny")
            .await
            .unwrap();
        assert!(db
            .authenticate_download_token(token)
            .await
            .unwrap()
            .is_none());
        db.set_download_permission("admin", "user", user, "inherit")
            .await
            .unwrap();
        assert!(db
            .authenticate_download_token(token)
            .await
            .unwrap()
            .is_some());
        sqlx::query("UPDATE download_tokens SET expires_at=0")
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(db
            .authenticate_download_token(token)
            .await
            .unwrap()
            .is_none());
        sqlx::query("UPDATE download_tokens SET expires_at=?")
            .bind(unix_timestamp() + 100)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE users SET disabled=1 WHERE id=?")
            .bind(user)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(db
            .authenticate_download_token(token)
            .await
            .unwrap()
            .is_none());
        db.revoke_download_token(user + 1, id).await.unwrap();
        assert_eq!(
            db.download_tokens(user)
                .await
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        db.revoke_download_token(user, id).await.unwrap();
        assert!(db
            .authenticate_download_token(token)
            .await
            .unwrap()
            .is_none());
    }
    #[tokio::test]
    async fn domain_rules_match_labels_and_preserve_audited_scope() {
        let (db, _) = fixture().await;
        db.set_download_block("admin", "example.org", true, "abuse", true)
            .await
            .unwrap();
        for d in ["example.org", "cdn.example.org", "a.b.example.org"] {
            assert!(db.download_blocked(d).await.unwrap());
        }
        for d in ["notexample.org", "example.org.evil.net"] {
            assert!(!db.download_blocked(d).await.unwrap());
        }
        db.set_download_block("admin", "example.org", false, "exact", true)
            .await
            .unwrap();
        assert!(!db.download_blocked("cdn.example.org").await.unwrap());
        db.set_download_block("admin", "example.org", false, "", false)
            .await
            .unwrap();
        assert!(!db.download_blocked("example.org").await.unwrap());
    }
    #[tokio::test]
    async fn chunks_enforce_daily_and_existing_monthly_budgets_atomically() {
        let (db, user) = fixture().await;
        let mut c = Config::default();
        c.download.daily_user_bytes = 10;
        db.download_event(user, "cdn.example.org", "example.org", false, false, &c)
            .await
            .unwrap();
        assert!(db
            .charge_download(user, "cdn.example.org", "example.org", 6, &c)
            .await
            .unwrap());
        assert!(!db
            .charge_download(user, "cdn.example.org", "example.org", 5, &c)
            .await
            .unwrap());
        assert!(db
            .charge_download(user, "cdn.example.org", "example.org", 4, &c)
            .await
            .unwrap());
        let stats = db.download_stats(1, None, None).await.unwrap();
        assert_eq!(stats["domains"][0]["bytes"], 10);
        assert_eq!(stats["rows"][0]["entry_domain"], "example.org");
        let (_, month) = crate::quota_period(&c.quota.timezone);
        let used: i64 =
            sqlx::query_scalar("SELECT response_bytes FROM traffic_monthly WHERE month=?")
                .bind(&month)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(used, 10);
        c.download.daily_user_bytes = 100;
        db.set_user_billing_profile("admin", user, None, "custom", Some(12))
            .await
            .unwrap();
        assert!(!db
            .charge_download(user, "cdn.example.org", "example.org", 3, &c)
            .await
            .unwrap());
        assert!(db
            .charge_download(user, "cdn.example.org", "example.org", 2, &c)
            .await
            .unwrap());
        let group = db
            .create_billing_group("admin", "Limited", Some(1))
            .await
            .unwrap()
            .unwrap();
        db.set_user_billing_profile("admin", user, Some(group.id), "unlimited", None)
            .await
            .unwrap();
        assert!(!db
            .charge_download(user, "cdn.example.org", "example.org", 2, &c)
            .await
            .unwrap());
        let used: i64 =
            sqlx::query_scalar("SELECT response_bytes FROM traffic_monthly WHERE month=?")
                .bind(month)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(
            used, 12,
            "failed group charge must roll back global and user charges"
        );
    }
    #[tokio::test]
    async fn concurrent_chunks_cannot_overspend_and_global_zero_blocks() {
        let (db, user) = fixture().await;
        let mut c = Config::default();
        c.download.daily_user_bytes = 10;
        let (a, b) = tokio::join!(
            db.charge_download(user, "a.example.org", "a.example.org", 6, &c),
            db.charge_download(user, "b.example.org", "b.example.org", 6, &c)
        );
        assert_ne!(a.unwrap(), b.unwrap());
        c.quota.enabled = true;
        c.quota.monthly_gb = 0;
        assert!(!db
            .charge_download(user, "a.example.org", "a.example.org", 1, &c)
            .await
            .unwrap());
    }
    #[tokio::test]
    async fn v4_upgrade_keeps_users_sessions_and_permissions_default_denied() {
        let (db, user) = fixture().await;
        let session = db.create_user_session(user, "test").await.unwrap().unwrap();
        for table in [
            "download_tokens",
            "download_permissions",
            "download_blocks",
            "download_usage",
        ] {
            // Table names are fixed fixture identifiers from the list above.
            sqlx::query(sqlx::AssertSqlSafe(format!("DROP TABLE {table}")))
                .execute(&db.pool)
                .await
                .unwrap();
        }
        sqlx::query("DELETE FROM schema_migrations WHERE version=5")
            .execute(&db.pool)
            .await
            .unwrap();
        db.migrate().await.unwrap();
        db.migrate().await.unwrap();
        assert!(db
            .authenticate_user_session(&session.token)
            .await
            .unwrap()
            .is_some());
        assert!(!db.download_allowed(user).await.unwrap());
        assert_eq!(db.health(":memory:").await.unwrap().schema_version, 5);
    }
}
