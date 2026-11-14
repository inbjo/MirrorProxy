use super::*;

#[tokio::test]
async fn sqlite_runtime_includes_the_mismatched_wal_page_fix() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    let parts = version
        .split('.')
        .map(|part| part.parse::<u32>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(parts.len(), 3);
    assert!(
        (parts[0], parts[1], parts[2]) >= (3, 54, 0),
        "SQLite {version} predates the RUSTSEC-2026-0459 fix"
    );
    database.pool.close().await;
}

#[tokio::test]
async fn argon2_verification_preserves_salts_and_rejects_invalid_credentials() {
    let password = random_secret(28);
    let other = random_secret(28);
    let first = hash_password_async(&password).await.unwrap();
    let second = hash_password_async(&password).await.unwrap();
    assert!(first.starts_with("$argon2id$"));
    assert_ne!(first, second, "each hash must have an independent salt");
    assert!(argon2_matches_async(&password, &first).await.unwrap());
    assert!(argon2_matches_async(&password, &second).await.unwrap());
    assert!(!argon2_matches_async(&other, &first).await.unwrap());
    assert!(!argon2_matches_async(&password, &other).await.unwrap());
}

#[tokio::test]
async fn login_sessions_use_independent_random_tokens_not_password_material() {
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.unwrap();
    let first = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let second = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.token.len(), 48);
    assert_eq!(second.token.len(), 48);
    assert_ne!(first.token, second.token);
    assert_ne!(first.token, credentials.password);
    assert_ne!(second.token, credentials.password);
    let hashes: Vec<String> = sqlx::query_scalar("SELECT token_hash FROM admin_sessions")
        .fetch_all(&database.pool)
        .await
        .unwrap();
    assert_eq!(hashes.len(), 2);
    assert!(hashes.contains(&hash_token(&first.token)));
    assert!(hashes.contains(&hash_token(&second.token)));
    assert!(!hashes.contains(&credentials.password));
}

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
    let wrong_password = random_secret(28);
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.expect("first database open should create admin");
    assert_eq!(credentials.username, "admin");
    assert!(database
        .login(&credentials.username, &wrong_password)
        .await
        .unwrap()
        .is_none());

    let session = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    assert!(database.authorize(&session.token).await.unwrap());
    database.logout(&session.token).await.unwrap();
    assert!(!database.authorize(&session.token).await.unwrap());
}

#[tokio::test]
async fn changing_password_revokes_existing_sessions() {
    let wrong_password = random_secret(28);
    let next_password = random_secret(28);
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.unwrap();
    let session = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    assert!(!database
        .change_admin_password(&credentials.username, &wrong_password, &next_password)
        .await
        .unwrap());
    assert!(database
        .change_admin_password(&credentials.username, &credentials.password, &next_password,)
        .await
        .unwrap());
    assert!(!database.authorize(&session.token).await.unwrap());
    assert!(database
        .login(&credentials.username, &next_password)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn locks_an_administrator_after_repeated_failures() {
    let wrong_password = random_secret(28);
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.unwrap();
    for attempt in 0..ADMIN_LOGIN_FAILURE_LIMIT {
        let outcome = database
            .login_with_context(&credentials.username, &wrong_password, "192.0.2.1")
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
            .login_with_context(&credentials.username, &credentials.password, "192.0.2.1")
            .await
            .unwrap(),
        AdminLoginOutcome::Locked { .. }
    ));
}

#[tokio::test]
async fn manages_multiple_admins_and_protects_the_last_super_admin() {
    let operator_password = random_secret(28);
    let duplicate_password = random_secret(28);
    let recovery_password = random_secret(28);
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let initial_username = credentials.unwrap().username;
    assert!(database
        .create_admin("admin", "operator", &operator_password, "admin")
        .await
        .unwrap());
    assert!(!database
        .create_admin("admin", "operator", &duplicate_password, "admin")
        .await
        .unwrap());
    assert!(!database
        .set_admin_disabled("admin", &initial_username, true)
        .await
        .unwrap());
    assert!(database
        .create_admin("admin", "recovery", &recovery_password, "super_admin",)
        .await
        .unwrap());
    assert!(database
        .set_admin_disabled("admin", "recovery", true)
        .await
        .unwrap());
    assert!(database
        .login("recovery", &recovery_password)
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
async fn invitation_history_is_limited_to_three_days_and_ten_rows() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    for index in 0..12 {
        database
            .create_email_invitation(
                "admin",
                &format!("person-{index}@example.com"),
                "Person",
                &format!("token-{index}"),
                unix_timestamp() + 600,
            )
            .await
            .unwrap();
    }
    assert_eq!(database.list_email_invitations().await.unwrap().len(), 10);

    sqlx::query("UPDATE email_invitations SET created_at = ?")
        .bind(unix_timestamp() - 4 * 24 * 60 * 60)
        .execute(&database.pool)
        .await
        .unwrap();
    assert!(database.list_email_invitations().await.unwrap().is_empty());
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
        .enqueue_email("PERSON@example.com", "Subject", "message body", None)
        .await
        .unwrap();
    let queued = database.pending_outbox(10).await.unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].recipient, "person@example.com");
    assert_eq!(queued[0].body, "message body");

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
async fn legacy_mode_keeps_secrets_readable_without_a_master_key() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    database
        .save_smtp_settings(
            "admin",
            &SmtpSettings {
                enabled: true,
                host: "smtp.example.com".to_string(),
                port: 587,
                security: "starttls".to_string(),
                username: Some("mailer".to_string()),
                password: Some("smtp-secret".to_string()),
                from_name: "MirrorProxy".to_string(),
                from_address: "mirror@example.com".to_string(),
            },
            false,
        )
        .await
        .unwrap();
    database
        .enqueue_email("person@example.com", "Subject", "plain body", None)
        .await
        .unwrap();

    let password: String = sqlx::query_scalar("SELECT password FROM smtp_settings")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    let body: String = sqlx::query_scalar("SELECT body FROM email_outbox")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(password, "smtp-secret");
    assert_eq!(body, "plain body");
}

#[tokio::test]
async fn master_key_encrypts_sensitive_database_fields_and_reads_them_back() {
    let (mut database, _) = Database::open(":memory:").await.unwrap();
    database.secrets = SecretCipher::for_tests();
    database
        .save_smtp_settings(
            "admin",
            &SmtpSettings {
                enabled: true,
                host: "smtp.example.com".to_string(),
                port: 587,
                security: "starttls".to_string(),
                username: Some("mailer".to_string()),
                password: Some("smtp-secret".to_string()),
                from_name: "MirrorProxy".to_string(),
                from_address: "mirror@example.com".to_string(),
            },
            false,
        )
        .await
        .unwrap();
    database
        .enqueue_email(
            "person@example.com",
            "Subject",
            "plain body",
            Some("<p>plain body</p>"),
        )
        .await
        .unwrap();
    let provider_id = database
        .save_auth_provider(
            "admin",
            &test_auth_provider("github", "oauth-secret"),
            false,
        )
        .await
        .unwrap();
    database
        .store_user_auth_flow(
            "state",
            provider_id,
            "flow-secret",
            "login",
            None,
            unix_timestamp() + 60,
        )
        .await
        .unwrap();

    for query in [
        "SELECT password FROM smtp_settings",
        "SELECT body FROM email_outbox",
        "SELECT client_secret FROM auth_providers",
        "SELECT payload FROM user_auth_flows",
    ] {
        let stored: String = sqlx::query_scalar(query)
            .fetch_one(&database.pool)
            .await
            .unwrap();
        assert!(stored.starts_with("enc:v1:"));
    }
    assert_eq!(
        database
            .smtp_settings()
            .await
            .unwrap()
            .unwrap()
            .password
            .as_deref(),
        Some("smtp-secret")
    );
    assert_eq!(
        database.pending_outbox(1).await.unwrap()[0].body,
        "plain body"
    );
    assert_eq!(
        database
            .auth_provider_by_id(provider_id)
            .await
            .unwrap()
            .unwrap()
            .client_secret
            .as_deref(),
        Some("oauth-secret")
    );
    assert_eq!(
        database
            .take_user_auth_flow("state")
            .await
            .unwrap()
            .unwrap()
            .payload,
        "flow-secret"
    );
}

#[tokio::test]
async fn records_schema_version_and_creates_consistent_backup() {
    let root = std::env::temp_dir().join(format!(
        "mirrorproxy-backup-test-{}-{}",
        std::process::id(),
        unix_timestamp()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("source.sqlite3");
    let backup = root.join("backup.sqlite3");
    let (database, _) = Database::open(source.to_str().unwrap()).await.unwrap();
    let health = database.health(source.to_str().unwrap()).await.unwrap();
    assert_eq!(health.integrity, "ok");
    assert_eq!(health.schema_version, CURRENT_SCHEMA_VERSION);
    database.backup(&backup).await.unwrap();
    database.pool.close().await;
    drop(database);
    let (restored, _) = Database::open(backup.to_str().unwrap()).await.unwrap();
    assert_eq!(
        restored
            .health(backup.to_str().unwrap())
            .await
            .unwrap()
            .integrity,
        "ok"
    );
    restored.pool.close().await;
    drop(restored);
    std::fs::remove_dir_all(root).unwrap();
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
    let operator_password = random_secret(28);
    let replacement_password = random_secret(28);
    let (database, _) = Database::open(":memory:").await.unwrap();
    database
        .create_admin("admin", "operator", &operator_password, "admin")
        .await
        .unwrap();
    let session = database
        .login("operator", &operator_password)
        .await
        .unwrap()
        .unwrap();
    assert!(database
        .reset_admin_password("admin", "operator", &replacement_password)
        .await
        .unwrap());
    assert!(!database.authorize(&session.token).await.unwrap());
    assert!(database
        .login("operator", &replacement_password)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn username_change_and_password_reset_follow_the_renamed_initial_admin() {
    let (mut database, credentials) = Database::open(":memory:").await.unwrap();
    database.secrets = SecretCipher::for_tests();
    let credentials = credentials.unwrap();
    let mut config = Config::default();
    config.webauthn.break_glass_username = credentials.username.clone();
    database
        .save_runtime_config("system", &config, "seed runtime configuration")
        .await
        .unwrap();
    let session = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        database
            .change_admin_username(
                &credentials.username,
                &credentials.password,
                "recovered-admin",
            )
            .await
            .unwrap(),
        AdminUsernameChangeOutcome::Changed
    );
    assert!(!database.authorize(&session.token).await.unwrap());
    assert!(database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .is_none());
    let recovered_session = database
        .login("recovered-admin", &credentials.password)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        database
            .load_or_seed_runtime_config(Config::default())
            .await
            .unwrap()
            .webauthn
            .break_glass_username,
        "recovered-admin"
    );
    let stored: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'runtime_config'")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert!(stored.starts_with("enc:"));
    let replacement = database
        .reset_initial_admin_password("cli")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replacement.username, "recovered-admin");
    assert_eq!(replacement.password.len(), 28);
    assert!(!database.authorize(&recovered_session.token).await.unwrap());
    assert!(database
        .login("recovered-admin", &credentials.password)
        .await
        .unwrap()
        .is_none());
    assert!(database
        .login("recovered-admin", &replacement.password)
        .await
        .unwrap()
        .is_some());
    assert!(database
        .audit_log_page(1, 10)
        .await
        .unwrap()
        .0
        .iter()
        .any(|entry| entry.action == "admin_password_reset"
            && entry.detail.contains("username=recovered-admin")));
}

#[tokio::test]
async fn username_change_rejects_an_existing_username_without_changing_credentials() {
    let existing_password = random_secret(28);
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.unwrap();
    database
        .create_admin("cli", "existing-admin", &existing_password, "admin")
        .await
        .unwrap();

    assert_eq!(
        database
            .change_admin_username(
                &credentials.username,
                &credentials.password,
                "existing-admin",
            )
            .await
            .unwrap(),
        AdminUsernameChangeOutcome::UsernameExists
    );
    assert!(database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn username_change_preserves_encrypted_config_for_an_unrelated_break_glass_admin() {
    let (mut database, credentials) = Database::open(":memory:").await.unwrap();
    database.secrets = SecretCipher::for_tests();
    let credentials = credentials.unwrap();
    let mut config = Config::default();
    config.webauthn.break_glass_username = "separate-break-glass".to_string();
    database
        .save_runtime_config("system", &config, "seed runtime configuration")
        .await
        .unwrap();
    let stored_before: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'runtime_config'")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert!(stored_before.starts_with("enc:"));

    assert_eq!(
        database
            .change_admin_username(
                &credentials.username,
                &credentials.password,
                "renamed-admin",
            )
            .await
            .unwrap(),
        AdminUsernameChangeOutcome::Changed
    );

    let stored_after: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'runtime_config'")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(stored_after, stored_before);
    assert_eq!(
        database
            .load_or_seed_runtime_config(Config::default())
            .await
            .unwrap()
            .webauthn
            .break_glass_username,
        "separate-break-glass"
    );
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
    config.outbound_proxy = crate::config::OutboundProxyConfig {
        enabled: true,
        url: "socks5h://proxy.example:1080".to_string(),
        no_proxy: vec!["localhost".to_string()],
        username: Some("proxy-user".to_string()),
        password: Some("proxy-password".to_string()),
    };
    database
        .save_runtime_config("admin", &config, "update runtime configuration")
        .await
        .unwrap();
    let loaded = database
        .load_or_seed_runtime_config(Config::default())
        .await
        .unwrap();
    assert_eq!(loaded.quota.monthly_gb, 42);
    assert_eq!(
        loaded.outbound_proxy.password.as_deref(),
        Some("proxy-password")
    );
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
            delivered_response_bytes: 1024,
            stream_error: false,
            reserved_bytes: 0,
            user_id: None,
            group_id: None,
            request_event_retention_days: 30,
            location: &GeoLocation::default(),
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
            delivered_response_bytes: 12,
            stream_error: false,
            reserved_bytes: 0,
            user_id: None,
            group_id: None,
            request_event_retention_days: 30,
            location: &GeoLocation::default(),
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
async fn aggregates_geo_traffic_without_storing_client_ips() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let location = GeoLocation {
        country: Some("中国".into()),
        province: Some("广东省".into()),
        city: Some("深圳市".into()),
        isp: Some("电信".into()),
        country_code: Some("CN".into()),
    };
    database
        .record_proxy_response(ProxyTrafficRecord {
            day: "2026-07-10",
            month: "2026-07",
            target_code: "npm",
            method: "GET",
            path: "/npm/react",
            status_code: 200,
            response_bytes: 2048,
            delivered_response_bytes: 1024,
            stream_error: false,
            reserved_bytes: 0,
            user_id: None,
            group_id: None,
            request_event_retention_days: 30,
            location: &location,
        })
        .await
        .unwrap();
    let report = database
        .geo_traffic_overview(
            "2026-07-01",
            "2026-07-31",
            Some("npm"),
            Some("CN"),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(report.request_count, 1);
    assert_eq!(report.response_bytes, 1024);
    assert_eq!(report.billed_bytes, 2048);
    assert_eq!(report.regions[0].city, "深圳市");
    let columns: Vec<String> = sqlx::query("PRAGMA table_info(geo_traffic_daily)")
        .fetch_all(&database.pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.try_get("name").unwrap())
        .collect();
    assert!(!columns.iter().any(|column| column.contains("ip")));
}

#[tokio::test]
async fn manages_normalized_ip_access_rules() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let rule = database
        .create_ip_access_rule("deny", "cidr", "203.0.113.0/24", "scanner", true)
        .await
        .unwrap();
    assert_eq!(
        database.list_ip_access_rules().await.unwrap(),
        vec![rule.clone()]
    );
    let updated = database
        .update_ip_access_rule(rule.id, "deny", "cidr", "203.0.113.0/24", "disabled", false)
        .await
        .unwrap()
        .unwrap();
    assert!(!updated.enabled);
    assert!(database.delete_ip_access_rule(rule.id).await.unwrap());
    assert!(database.list_ip_access_rules().await.unwrap().is_empty());
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
            delivered_response_bytes: 1,
            stream_error: false,
            reserved_bytes: 0,
            user_id: None,
            group_id: None,
            request_event_retention_days: 1,
            location: &GeoLocation::default(),
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
            delivered_response_bytes: 4,
            stream_error: false,
            reserved_bytes: 6,
            user_id: None,
            group_id: None,
            request_event_retention_days: 30,
            location: &GeoLocation::default(),
        })
        .await
        .unwrap();
    assert!(database
        .try_reserve_monthly_bytes("2026-07", 10, 6)
        .await
        .unwrap());
}

#[tokio::test]
async fn public_and_user_reservations_share_the_global_traffic_limit() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let user = database
        .create_user("admin", "shared-limit@example.com", "Shared Limit", 12)
        .await
        .unwrap()
        .unwrap();

    assert!(database
        .try_reserve_monthly_bytes("2026-07", 10, 6)
        .await
        .unwrap());
    assert_eq!(
        database
            .try_reserve_hierarchical_bytes("2026-07", user.id, Some(10), Some(8), 5)
            .await
            .unwrap(),
        HierarchicalReservationOutcome::Exceeded { scope: "global" }
    );
    assert_eq!(
        database
            .try_reserve_hierarchical_bytes("2026-07", user.id, Some(10), Some(8), 4)
            .await
            .unwrap(),
        HierarchicalReservationOutcome::Reserved { group_id: None }
    );
}

#[tokio::test]
async fn team_target_access_defaults_open_and_enforces_an_allowlist() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let user = database
        .create_user("admin", "team@example.com", "Team User", 12)
        .await
        .unwrap()
        .unwrap();
    let group = database
        .create_billing_group("admin", "Restricted Team", None)
        .await
        .unwrap()
        .unwrap();
    database
        .set_user_billing_profile("admin", user.id, Some(group.id), "default", None)
        .await
        .unwrap();
    assert!(database.user_target_allowed(user.id, "npm").await.unwrap());
    database
        .set_group_target_access("admin", group.id, &["npm".to_string()])
        .await
        .unwrap();
    assert!(database.user_target_allowed(user.id, "npm").await.unwrap());
    assert!(!database.user_target_allowed(user.id, "pypi").await.unwrap());
    assert_eq!(
        database
            .group_target_access(group.id)
            .await
            .unwrap()
            .unwrap(),
        GroupTargetAccess {
            group_id: group.id,
            target_codes: vec!["npm".to_string()],
        }
    );
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
            delivered_response_bytes: 4,
            stream_error: false,
            reserved_bytes: 6,
            user_id: Some(first.id),
            group_id: Some(group.id),
            request_event_retention_days: 30,
            location: &GeoLocation::default(),
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
            delivered_response_bytes: 4,
            stream_error: false,
            reserved_bytes: 4,
            user_id: Some(first.id),
            group_id: Some(group.id),
            request_event_retention_days: 30,
            location: &GeoLocation::default(),
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
async fn paginates_audit_log() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    for _ in 0..3 {
        database
            .save_runtime_config("admin", &Config::default(), "update runtime configuration")
            .await
            .unwrap();
    }

    let (entries, total) = database.audit_log_page(2, 2).await.unwrap();
    assert_eq!(total, 3);
    assert_eq!(entries.len(), 1);
    assert!(entries.iter().any(|entry| entry.username == "admin"
        && entry.action == "update runtime configuration"
        && entry.detail == "runtime_config"));
}

#[tokio::test]
async fn stores_provider_secrets_and_preserves_them_on_update() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let mut provider = test_auth_provider("github", "plain-secret");
    let id = database
        .save_auth_provider("admin", &provider, false)
        .await
        .unwrap();
    provider.id = id;
    provider.display_name = "GitHub Enterprise".to_string();
    provider.client_secret = None;
    database
        .save_auth_provider("admin", &provider, true)
        .await
        .unwrap();

    let stored = database.auth_provider_by_id(id).await.unwrap().unwrap();
    assert_eq!(stored.display_name, "GitHub Enterprise");
    assert_eq!(stored.client_secret.as_deref(), Some("plain-secret"));
    let serialized = serde_json::to_string(&stored).unwrap();
    assert!(!serialized.contains("plain-secret"));
    let database_value: String =
        sqlx::query_scalar("SELECT client_secret FROM auth_providers WHERE id = ?")
            .bind(id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(database_value, "plain-secret");
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
            "plain-flow",
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
            .payload,
        "plain-flow"
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
    provider.client_secret = None;
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
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let second = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let sessions = database
        .list_admin_sessions(&credentials.username, &first.token)
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
        .revoke_admin_session("admin", &credentials.username, &second_id)
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
    let deleted_at: Option<i64> = sqlx::query_scalar("SELECT deleted_at FROM users WHERE id = ?")
        .bind(user.id)
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert!(deleted_at.is_some());
}

#[tokio::test]
async fn persists_latest_source_health_by_target() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    database
        .replace_source_health(&[SourceHealthRecord {
            target_code: "maven".to_string(),
            adapter: "maven".to_string(),
            status: "unhealthy".to_string(),
            http_status: Some(403),
            latency_ms: Some(512),
            checked_at: 1_721_880_000,
            error: Some("HTTP 403".to_string()),
            endpoints: vec![SourceEndpointHealthRecord {
                position: 0,
                endpoint: "https://repo.example/maven2".to_string(),
                status: "unhealthy".to_string(),
                http_status: Some(403),
                latency_ms: Some(512),
                checked_at: 1_721_880_000,
                error: Some("HTTP 403".to_string()),
            }],
        }])
        .await
        .unwrap();

    let records = database.source_health().await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].target_code, "maven");
    assert_eq!(records[0].http_status, Some(403));
    assert_eq!(records[0].latency_ms, Some(512));
    assert_eq!(records[0].endpoints.len(), 1);
    assert_eq!(
        records[0].endpoints[0].endpoint,
        "https://repo.example/maven2"
    );
}

#[tokio::test]
async fn stores_acme_secrets_separately_and_reloads_them() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let settings = AcmeConfig {
        enabled: true,
        email: "admin@example.com".to_string(),
        domains: vec!["example.com".to_string(), "*.example.com".to_string()],
        challenge: "dns-01".to_string(),
        dns: crate::config::AcmeDnsConfig {
            provider: "cloudflare".to_string(),
            cloudflare_zone_id: "zone-id".to_string(),
            cloudflare_api_token: "cloudflare-secret".to_string(),
            ..crate::config::AcmeDnsConfig::default()
        },
        ..AcmeConfig::default()
    };

    database
        .save_acme_settings("admin", &settings)
        .await
        .unwrap();
    let stored = database.acme_settings().await.unwrap().unwrap();
    assert_eq!(stored, settings);

    let settings_json: String =
        sqlx::query_scalar("SELECT settings_json FROM acme_settings WHERE singleton = 1")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert!(!settings_json.contains("cloudflare-secret"));
    let secret: String =
        sqlx::query_scalar("SELECT cloudflare_api_token FROM acme_settings WHERE singleton = 1")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(secret, "cloudflare-secret");
}

fn test_auth_provider(slug: &str, client_secret: &str) -> AuthProvider {
    AuthProvider {
        id: 0,
        slug: slug.to_string(),
        display_name: "GitHub".to_string(),
        kind: "oauth2".to_string(),
        preset: "github".to_string(),
        enabled: true,
        client_id: "client-id".to_string(),
        client_secret: Some(client_secret.to_string()),
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
