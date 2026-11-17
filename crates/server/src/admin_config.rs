//! Authenticated runtime configuration reads and serialized, versioned updates.
use super::*;

pub(super) async fn admin_config(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => {
            let runtime = state.runtime.read().unwrap();
            (
                [
                    (header::ETAG, format!("\"{}\"", runtime.revision)),
                    (header::CACHE_CONTROL, "no-store".into()),
                ],
                Json(admin_config_value((*runtime.config).clone())),
            )
                .into_response()
        }
        Ok(false) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            internal_error_response()
        }
    }
}

pub(super) async fn update_admin_config(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(next): Json<Config>,
) -> Response {
    // Finish persist + publish even if the saving browser disconnects.
    let pending = state.database.tasks.enter();
    tokio::spawn(async move {
        let _pending = pending;
        update_admin_config_inner(headers, State(state), Json(next)).await
    })
    .await
    .unwrap_or_else(|_| internal_error_response())
}

pub(super) async fn update_admin_config_inner(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(mut next_config): Json<Config>,
) -> Response {
    let identity = match authenticated_admin(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return internal_error_response();
        }
    };

    let _update = state.config_update.lock().await;
    let expected = headers.get(header::IF_MATCH).and_then(|v| v.to_str().ok());
    let revision = format!("\"{}\"", state.runtime.read().unwrap().revision);
    if headers.contains_key(header::IF_MATCH) && expected != Some(revision.as_str()) {
        return (StatusCode::PRECONDITION_FAILED, Json(serde_json::json!({"error":"configuration changed; reload before saving","code":"config_conflict"}))).into_response();
    }
    let current = state.config();
    if next_config.download != current.download && identity.role != "super_admin" {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error":"super administrator access required"})),
        )
            .into_response();
    }
    if next_config.alerts.webhook_url.trim().is_empty()
        && !current.alerts.webhook_url.trim().is_empty()
    {
        next_config.alerts.webhook_url = current.alerts.webhook_url.clone();
    }
    if next_config
        .outbound_proxy
        .username
        .as_deref()
        .is_none_or(|username| username.trim().is_empty())
    {
        next_config.outbound_proxy.username = None;
        next_config.outbound_proxy.password = None;
    } else if next_config.outbound_proxy.password.is_none()
        && next_config.outbound_proxy.username == current.outbound_proxy.username
    {
        next_config.outbound_proxy.password = current.outbound_proxy.password.clone();
    }
    // ACME settings use a dedicated super-admin endpoint so secrets never pass
    // through the general runtime configuration API.
    next_config.acme = current.acme.clone();
    if next_config.alerts.enabled && next_config.alerts.email_enabled {
        match state.database.smtp_settings().await {
            Ok(Some(settings)) if settings.enabled => {}
            Ok(_) => {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "SMTP must be enabled before email alerts can be enabled"
                    })),
                )
                    .into_response();
            }
            Err(error) => {
                tracing::error!(%error, "failed to validate SMTP for email alerts");
                return internal_error_response();
            }
        }
    }
    if let Err(error) = next_config.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response();
    }
    // Private per-upstream credentials remain service-owned.
    next_config.upstream_auth = current.upstream_auth.clone();
    if next_config.listen_addr != current.listen_addr
        || next_config.management != current.management
        || next_config.database_path != current.database_path
    {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "listener addresses and database_path cannot be changed through the runtime API; update the service configuration and restart"
            })),
        )
            .into_response();
    }
    let next_webauthn = if next_config.webauthn != current.webauthn {
        if next_config.webauthn.require_passkey {
            match state
                .database
                .admins_without_minimum_passkeys(2, &next_config.webauthn.break_glass_username)
                .await
            {
                Ok(admins) if admins.is_empty() => {}
                Ok(admins) => {
                    return conflict_response(&format!(
                        "cannot require passkeys until every non-break-glass administrator has two passkeys: {}",
                        admins.join(", ")
                    ));
                }
                Err(error) => {
                    tracing::error!(%error, "failed to verify administrator passkey readiness");
                    return internal_error_response();
                }
            }
        }
        match build_webauthn(&next_config) {
            Ok(webauthn) => Some(webauthn),
            Err(error) => return bad_request_response(error.to_string()),
        }
    } else {
        None
    };
    let next_client = if next_config.timeout.request_secs != current.timeout.request_secs
        || next_config.outbound_proxy != current.outbound_proxy
        || next_config.upstream_tls != current.upstream_tls
    {
        match build_upstream_client(&next_config) {
            Ok(client) => Some(client),
            Err(error) => return bad_request_response(error.to_string()),
        }
    } else {
        None
    };
    let restart_required = if next_config.geoip != current.geoip {
        vec!["geoip"]
    } else {
        Vec::new()
    };
    if let Err(error) = state
        .database
        .save_runtime_config(
            &identity.username,
            &next_config,
            "update runtime configuration",
        )
        .await
    {
        tracing::error!(%error, "failed to save runtime configuration");
        return internal_error_response();
    }
    if next_config.download != current.download {
        state.downloads.cancel_all();
    }
    state.upstream_selector.configure(&next_config);
    {
        let mut runtime = state.runtime.write().unwrap();
        runtime.config = Arc::new(next_config.clone());
        if let Some(client) = next_client {
            runtime.client = client;
        }
        if let Some(webauthn) = next_webauthn {
            runtime.webauthn = webauthn;
        }
        runtime.revision = uuid::Uuid::new_v4().to_string();
    }
    log_upstream_tls_configuration(&next_config);
    let revision = format!("\"{}\"", state.runtime.read().unwrap().revision);
    (
        [(header::ETAG, revision)],
        Json(AdminConfigUpdateResponse {
            config: admin_config_value(next_config),
            restart_required,
        }),
    )
        .into_response()
}

pub(super) fn admin_config_value(mut config: Config) -> serde_json::Value {
    let has_password = config.outbound_proxy.password.is_some();
    let has_alert_webhook = !config.alerts.webhook_url.is_empty();
    config.outbound_proxy.password = None;
    config.alerts.webhook_url.clear();
    let mut value = serde_json::to_value(config).expect("configuration is serializable");
    if let Some(outbound_proxy) = value
        .get_mut("outbound_proxy")
        .and_then(serde_json::Value::as_object_mut)
    {
        outbound_proxy.insert("has_password".into(), has_password.into());
    }
    if let Some(alerts) = value
        .get_mut("alerts")
        .and_then(serde_json::Value::as_object_mut)
    {
        alerts.insert("has_webhook_url".into(), has_alert_webhook.into());
    }
    value
}
