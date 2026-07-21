use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{connect_info::ConnectInfo, Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    admin_token, bad_request_response,
    database::{Database, SmtpSettings},
    internal_error_response, require_super_admin, unauthorized_response, user_session_cookie,
    valid_user_email, AppState, SecretCipher,
};

#[derive(Serialize)]
struct PublicSmtpSettings {
    enabled: bool,
    host: String,
    port: u16,
    security: String,
    username: Option<String>,
    has_password: bool,
    from_name: String,
    from_address: String,
    master_key_configured: bool,
}

#[derive(Deserialize)]
pub(crate) struct UpdateSmtpSettingsRequest {
    enabled: bool,
    host: String,
    port: u16,
    security: String,
    username: Option<String>,
    password: Option<String>,
    from_name: String,
    from_address: String,
}

pub(crate) async fn get_smtp_settings(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    match state.database.smtp_settings().await {
        Ok(settings) => {
            let settings = settings.unwrap_or_else(default_smtp_settings);
            Json(PublicSmtpSettings {
                enabled: settings.enabled,
                host: settings.host,
                port: settings.port,
                security: settings.security,
                username: settings.username,
                has_password: settings.encrypted_password.is_some(),
                from_name: settings.from_name,
                from_address: settings.from_address,
                master_key_configured: state.master_key.is_some(),
            })
            .into_response()
        }
        Err(error) => {
            tracing::error!(%error, "failed to load SMTP settings");
            internal_error_response()
        }
    }
}

pub(crate) async fn update_smtp_settings(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<UpdateSmtpSettingsRequest>,
) -> Response {
    let identity = match require_recent_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if let Err(error) = validate_smtp_request(&request) {
        return bad_request_response(error.to_string());
    }
    let encrypted_password = match request.password.as_deref() {
        Some(password) if !password.is_empty() => {
            let Some(cipher) = state.master_key.as_deref() else {
                return conflict(
                    "MIRRORPROXY_MASTER_KEY is required before saving SMTP credentials",
                );
            };
            match cipher.encrypt("smtp-password", password.as_bytes()) {
                Ok(value) => Some(value),
                Err(error) => {
                    tracing::error!(%error, "failed to encrypt SMTP password");
                    return internal_error_response();
                }
            }
        }
        _ => None,
    };
    if request.enabled && state.master_key.is_none() {
        return conflict("MIRRORPROXY_MASTER_KEY is required before enabling email delivery");
    }
    let settings = SmtpSettings {
        enabled: request.enabled,
        host: request.host.trim().to_string(),
        port: request.port,
        security: request.security,
        username: request.username.filter(|value| !value.trim().is_empty()),
        encrypted_password,
        from_name: request.from_name.trim().to_string(),
        from_address: request.from_address.trim().to_ascii_lowercase(),
    };
    let preserve_password = request.password.as_deref().is_none_or(str::is_empty);
    match state
        .database
        .save_smtp_settings(&identity.username, &settings, preserve_password)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to save SMTP settings");
            internal_error_response()
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct TestSmtpRequest {
    recipient: String,
}

pub(crate) async fn test_smtp_settings(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<TestSmtpRequest>,
) -> Response {
    if let Err(response) = require_recent_super_admin(&headers, &state).await {
        return response;
    }
    if !valid_user_email(request.recipient.trim()) {
        return bad_request_response("a valid recipient email is required".to_string());
    }
    match enqueue_plain_email(
        &state,
        request.recipient.trim(),
        "MirrorProxy SMTP test",
        "MirrorProxy email delivery is configured correctly.",
    )
    .await
    {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(response) => response,
    }
}

#[derive(Deserialize)]
pub(crate) struct CreateInvitationRequest {
    email: String,
    display_name: String,
    #[serde(default = "default_invitation_hours")]
    expires_in_hours: u32,
}

fn default_invitation_hours() -> u32 {
    72
}

pub(crate) async fn list_invitations(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    match state.database.list_email_invitations().await {
        Ok(invitations) => Json(invitations).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to list email invitations");
            internal_error_response()
        }
    }
}

pub(crate) async fn create_invitation(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<CreateInvitationRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let email = request.email.trim().to_ascii_lowercase();
    let display_name = request.display_name.trim();
    if !valid_user_email(&email)
        || display_name.is_empty()
        || display_name.chars().count() > 100
        || !(1..=24 * 30).contains(&request.expires_in_hours)
    {
        return bad_request_response("invalid invitation fields".to_string());
    }
    let token = random_token(48);
    let expires_at = now() + i64::from(request.expires_in_hours) * 60 * 60;
    let id = match state
        .database
        .create_email_invitation(&identity.username, &email, display_name, &token, expires_at)
        .await
    {
        Ok(id) => id,
        Err(error) => {
            tracing::error!(%error, "failed to create email invitation");
            return internal_error_response();
        }
    };
    let link = match login_link(
        &state.config().public_base_url,
        &email,
        "invitation",
        &token,
    ) {
        Ok(link) => link,
        Err(error) => {
            tracing::error!(%error, "failed to build invitation URL");
            return internal_error_response();
        }
    };
    let body =
        format!("You were invited to MirrorProxy. Open this link before it expires:\n\n{link}");
    match enqueue_plain_email(&state, &email, "MirrorProxy invitation", &body).await {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response(),
        Err(response) => response,
    }
}

pub(crate) async fn revoke_invitation(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    match state
        .database
        .revoke_email_invitation(&identity.username, id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "invitation not found" })),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to revoke invitation");
            internal_error_response()
        }
    }
}

pub(crate) async fn resend_invitation(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let token = random_token(48);
    let invitation = match state
        .database
        .renew_email_invitation(&identity.username, id, &token, now() + 72 * 60 * 60)
        .await
    {
        Ok(Some(invitation)) => invitation,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "invitation not found" })),
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!(%error, "failed to renew invitation");
            return internal_error_response();
        }
    };
    let link = match login_link(
        &state.config().public_base_url,
        &invitation.email,
        "invitation",
        &token,
    ) {
        Ok(link) => link,
        Err(error) => {
            tracing::error!(%error, "failed to build invitation URL");
            return internal_error_response();
        }
    };
    let body =
        format!("You were invited to MirrorProxy. Open this link before it expires:\n\n{link}");
    match enqueue_plain_email(&state, &invitation.email, "MirrorProxy invitation", &body).await {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(response) => response,
    }
}

#[derive(Deserialize)]
pub(crate) struct RequestEmailLogin {
    pub(crate) email: String,
    pub(crate) invitation_token: Option<String>,
}

pub(crate) async fn request_email_login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(request): Json<RequestEmailLogin>,
) -> Response {
    let email = request.email.trim().to_ascii_lowercase();
    if !valid_user_email(&email) {
        return StatusCode::ACCEPTED.into_response();
    }
    if state.master_key.is_none() {
        return service_unavailable("email login is not configured");
    }
    let existing = match state.database.user_by_email(&email).await {
        Ok(user) => user,
        Err(error) => {
            tracing::error!(%error, "failed to resolve email login user");
            return internal_error_response();
        }
    };
    let config = state.config();
    let invitation_id = if existing.is_some() {
        None
    } else {
        match config.registration.mode.as_str() {
            "open" => None,
            "domain_allowlist"
                if email_domain_allowed(&email, &config.registration.allowed_email_domains) =>
            {
                None
            }
            "invite_only" => {
                let Some(token) = request.invitation_token.as_deref() else {
                    return StatusCode::ACCEPTED.into_response();
                };
                match state.database.valid_invitation(&email, token).await {
                    Ok(Some(id)) => Some(id),
                    Ok(None) => return StatusCode::ACCEPTED.into_response(),
                    Err(error) => {
                        tracing::error!(%error, "failed to validate invitation");
                        return internal_error_response();
                    }
                }
            }
            _ => return StatusCode::ACCEPTED.into_response(),
        }
    };
    match state
        .database
        .allow_email_send(&email, &peer.ip().to_string())
        .await
    {
        Ok(true) => {}
        Ok(false) => return StatusCode::ACCEPTED.into_response(),
        Err(error) => {
            tracing::error!(%error, "email login send rate check failed");
            return internal_error_response();
        }
    }
    let token = random_token(48);
    let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000));
    let expires_at = now() + i64::from(config.registration.email_token_ttl_minutes) * 60;
    if let Err(error) = state
        .database
        .store_email_login_token(&email, &token, &code, invitation_id, expires_at)
        .await
    {
        tracing::error!(%error, "failed to store email login token");
        return internal_error_response();
    }
    let link = match login_link(&config.public_base_url, &email, "token", &token) {
        Ok(link) => link,
        Err(error) => {
            tracing::error!(%error, "failed to build email login URL");
            return internal_error_response();
        }
    };
    let body = format!(
        "Your MirrorProxy verification code is {code}.\n\nOr use this one-time link:\n{link}"
    );
    match enqueue_plain_email(&state, &email, "MirrorProxy sign in", &body).await {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(response) => response,
    }
}

#[derive(Deserialize)]
pub(crate) struct VerifyEmailLogin {
    pub(crate) email: String,
    pub(crate) code: Option<String>,
    pub(crate) token: Option<String>,
}

pub(crate) async fn verify_email_login(
    State(state): State<AppState>,
    Json(request): Json<VerifyEmailLogin>,
) -> Response {
    let email = request.email.trim().to_ascii_lowercase();
    let (credential, is_code) = match (request.code.as_deref(), request.token.as_deref()) {
        (Some(code), None) if code.len() == 6 => (code, true),
        (None, Some(token)) if !token.is_empty() => (token, false),
        _ => return unauthorized_response(),
    };
    let invitation_id = match state
        .database
        .consume_email_login_token(&email, credential, is_code)
        .await
    {
        Ok(Some(invitation_id)) => invitation_id,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "failed to consume email login token");
            return internal_error_response();
        }
    };
    let user = match state.database.user_by_email(&email).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let display_name = match invitation_id {
                Some(id) => match state.database.email_invitation(id).await {
                    Ok(Some(invitation))
                        if invitation.email == email
                            && invitation.status == "pending"
                            && invitation.expires_at > now() =>
                    {
                        invitation.display_name
                    }
                    _ => return unauthorized_response(),
                },
                None => email.split('@').next().unwrap_or("user").to_string(),
            };
            match state
                .database
                .create_user(
                    "email",
                    &email,
                    &display_name,
                    state.config().user_access.routing_id_min_length,
                )
                .await
            {
                Ok(Some(user)) => user,
                Ok(None) => match state.database.user_by_email(&email).await {
                    Ok(Some(user)) => user,
                    _ => return internal_error_response(),
                },
                Err(error) => {
                    tracing::error!(%error, "failed to create email user");
                    return internal_error_response();
                }
            }
        }
        Err(error) => {
            tracing::error!(%error, "failed to load email user");
            return internal_error_response();
        }
    };
    if let Some(id) = invitation_id {
        if let Err(error) = state.database.accept_email_invitation(id).await {
            tracing::error!(%error, "failed to accept invitation");
            return internal_error_response();
        }
    }
    let session = match state.database.create_user_session(user.id, "email").await {
        Ok(Some(session)) => session,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "failed to create user session");
            return internal_error_response();
        }
    };
    let mut response = Json(serde_json::json!({
        "user_id": session.identity.user_id,
        "email": session.identity.email,
        "display_name": session.identity.display_name,
        "expires_at": session.expires_at,
    }))
    .into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, user_session_cookie(&session.token));
    response
}

async fn require_recent_super_admin(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<crate::database::AdminIdentity, Response> {
    let identity = require_super_admin(headers, state).await?;
    let Some(token) = admin_token(headers) else {
        return Err(unauthorized_response());
    };
    match state.database.is_recent_admin_session(token).await {
        Ok(true) => Ok(identity),
        Ok(false) => Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "recent administrator verification required" })),
        )
            .into_response()),
        Err(error) => {
            tracing::error!(%error, "administrator recent verification query failed");
            Err(internal_error_response())
        }
    }
}

fn login_link(
    base_url: &str,
    email: &str,
    credential_name: &str,
    credential: &str,
) -> anyhow::Result<String> {
    let mut link = Url::parse(base_url)?;
    link.set_path("/login");
    link.set_query(None);
    link.set_fragment(None);
    link.query_pairs_mut()
        .append_pair("email", email)
        .append_pair(credential_name, credential);
    Ok(link.into())
}

fn validate_smtp_request(request: &UpdateSmtpSettingsRequest) -> anyhow::Result<()> {
    if request.enabled && (request.host.trim().is_empty() || request.from_address.trim().is_empty())
    {
        anyhow::bail!("SMTP host and from address are required when enabled");
    }
    if request.port == 0 || !matches!(request.security.as_str(), "starttls" | "smtps" | "none") {
        anyhow::bail!("SMTP port and security mode are invalid");
    }
    if !request.from_address.is_empty() && !valid_user_email(request.from_address.trim()) {
        anyhow::bail!("SMTP from address is invalid");
    }
    if request.from_name.trim().is_empty() || request.from_name.chars().count() > 100 {
        anyhow::bail!("SMTP from name must contain 1 to 100 characters");
    }
    Ok(())
}

fn default_smtp_settings() -> SmtpSettings {
    SmtpSettings {
        enabled: false,
        host: String::new(),
        port: 587,
        security: "starttls".to_string(),
        username: None,
        encrypted_password: None,
        from_name: "MirrorProxy".to_string(),
        from_address: String::new(),
    }
}

async fn enqueue_plain_email(
    state: &AppState,
    recipient: &str,
    subject: &str,
    body: &str,
) -> Result<(), Response> {
    let Some(cipher) = state.master_key.as_deref() else {
        return Err(service_unavailable("email delivery is not configured"));
    };
    match state.database.smtp_settings().await {
        Ok(Some(settings)) if settings.enabled => {}
        Ok(_) => return Err(service_unavailable("email delivery is not configured")),
        Err(error) => {
            tracing::error!(%error, "failed to load SMTP settings");
            return Err(internal_error_response());
        }
    }
    let encrypted = cipher
        .encrypt("email-outbox", body.as_bytes())
        .map_err(|error| {
            tracing::error!(%error, "failed to encrypt email outbox body");
            internal_error_response()
        })?;
    state
        .database
        .enqueue_email(recipient, subject, &encrypted)
        .await
        .map_err(|error| {
            tracing::error!(%error, "failed to enqueue email");
            internal_error_response()
        })?;
    Ok(())
}

pub(crate) fn spawn_email_outbox_worker(database: Arc<Database>, cipher: Arc<SecretCipher>) {
    tokio::spawn(async move {
        loop {
            if let Err(error) = process_email_outbox(&database, &cipher).await {
                tracing::warn!(%error, "email outbox processing failed");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

async fn process_email_outbox(database: &Database, cipher: &SecretCipher) -> anyhow::Result<()> {
    let Some(settings) = database
        .smtp_settings()
        .await?
        .filter(|settings| settings.enabled)
    else {
        return Ok(());
    };
    for item in database.pending_outbox(10).await? {
        let result = async {
            let body = cipher.decrypt("email-outbox", &item.encrypted_body)?;
            let body = String::from_utf8(body)?;
            send_email(&settings, cipher, &item.recipient, &item.subject, body).await
        }
        .await;
        if result.is_ok() {
            database.mark_outbox_sent(item.id).await?;
        } else {
            database
                .mark_outbox_failed(item.id, item.attempts + 1, "SMTP delivery failed")
                .await?;
        }
    }
    Ok(())
}

async fn send_email(
    settings: &SmtpSettings,
    cipher: &SecretCipher,
    recipient: &str,
    subject: &str,
    body: String,
) -> anyhow::Result<()> {
    let mut builder = match settings.security.as_str() {
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.host)?,
        "smtps" => AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.host)?,
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&settings.host),
        _ => anyhow::bail!("invalid SMTP security mode"),
    }
    .port(settings.port);
    if let (Some(username), Some(encrypted_password)) =
        (&settings.username, &settings.encrypted_password)
    {
        let password = String::from_utf8(cipher.decrypt("smtp-password", encrypted_password)?)?;
        builder = builder.credentials(Credentials::new(username.clone(), password));
    }
    let from: Mailbox = format!("{} <{}>", settings.from_name, settings.from_address).parse()?;
    let message = Message::builder()
        .from(from)
        .to(recipient.parse()?)
        .subject(subject)
        .body(body)?;
    builder.build().send(message).await?;
    Ok(())
}

fn email_domain_allowed(email: &str, domains: &[String]) -> bool {
    email
        .rsplit_once('@')
        .is_some_and(|(_, domain)| domains.iter().any(|allowed| allowed == domain))
}

fn random_token(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before epoch")
        .as_secs() as i64
}

fn conflict(message: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

fn service_unavailable(message: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_links_percent_encode_email_and_replace_existing_query() {
        let link = login_link(
            "https://mirror.example/base?old=value#fragment",
            "person+tag@example.com",
            "token",
            "one time token",
        )
        .unwrap();
        assert_eq!(
            link,
            "https://mirror.example/login?email=person%2Btag%40example.com&token=one+time+token"
        );
    }

    #[test]
    fn public_smtp_settings_never_serialize_a_password() {
        let rendered = serde_json::to_string(&PublicSmtpSettings {
            enabled: true,
            host: "smtp.example.com".to_string(),
            port: 587,
            security: "starttls".to_string(),
            username: Some("mailer".to_string()),
            has_password: true,
            from_name: "MirrorProxy".to_string(),
            from_address: "mirror@example.com".to_string(),
            master_key_configured: true,
        })
        .unwrap();
        assert!(!rendered.contains("smtp-secret"));
        assert!(!rendered.contains("encrypted_password"));
        assert!(!rendered.contains("\"password\""));
        assert!(rendered.contains("\"has_password\":true"));
    }
}
