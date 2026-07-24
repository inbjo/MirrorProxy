use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context};
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use oauth2::{
    basic::BasicClient, AuthType, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse as _, TokenUrl,
};
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata, CoreUserInfoClaims},
    AccessTokenHash, ClientId as OidcClientId, ClientSecret as OidcClientSecret,
    CsrfToken as OidcCsrfToken, IssuerUrl, Nonce, PkceCodeChallenge as OidcPkceCodeChallenge,
    PkceCodeVerifier as OidcPkceCodeVerifier, RedirectUrl as OidcRedirectUrl, Scope as OidcScope,
    TokenResponse as _,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::{
    authenticated_user, bad_request_response,
    database::{AuthProvider, ExternalRegistration},
    internal_error_response, require_super_admin, unauthorized_response, user_session_cookie,
    valid_user_email, AppState,
};

const FLOW_COOKIE: &str = "mirrorproxy_oauth_state";
const FLOW_LIFETIME_SECS: i64 = 10 * 60;

#[derive(Clone, Serialize)]
struct ProviderView {
    id: i64,
    slug: String,
    display_name: String,
    kind: String,
    preset: String,
    enabled: bool,
    client_id: String,
    has_client_secret: bool,
    issuer_url: Option<String>,
    authorization_url: Option<String>,
    token_url: Option<String>,
    userinfo_url: Option<String>,
    emails_url: Option<String>,
    scopes: Vec<String>,
    subject_field: String,
    email_field: String,
    email_verified_field: Option<String>,
    display_name_field: String,
    allow_registration: bool,
    auto_link_by_email: bool,
}

impl From<AuthProvider> for ProviderView {
    fn from(provider: AuthProvider) -> Self {
        Self {
            id: provider.id,
            slug: provider.slug,
            display_name: provider.display_name,
            kind: provider.kind,
            preset: provider.preset,
            enabled: provider.enabled,
            client_id: provider.client_id,
            has_client_secret: provider.client_secret.is_some(),
            issuer_url: provider.issuer_url,
            authorization_url: provider.authorization_url,
            token_url: provider.token_url,
            userinfo_url: provider.userinfo_url,
            emails_url: provider.emails_url,
            scopes: provider.scopes,
            subject_field: provider.subject_field,
            email_field: provider.email_field,
            email_verified_field: provider.email_verified_field,
            display_name_field: provider.display_name_field,
            allow_registration: provider.allow_registration,
            auto_link_by_email: provider.auto_link_by_email,
        }
    }
}

#[derive(Serialize)]
struct PublicProvider {
    slug: String,
    display_name: String,
    kind: String,
    allow_registration: bool,
}

#[derive(Deserialize)]
pub(crate) struct ProviderRequest {
    slug: String,
    display_name: String,
    kind: String,
    #[serde(default = "default_preset")]
    preset: String,
    #[serde(default)]
    enabled: bool,
    client_id: String,
    client_secret: Option<String>,
    issuer_url: Option<String>,
    authorization_url: Option<String>,
    token_url: Option<String>,
    userinfo_url: Option<String>,
    emails_url: Option<String>,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default = "default_subject_field")]
    subject_field: String,
    #[serde(default = "default_email_field")]
    email_field: String,
    email_verified_field: Option<String>,
    #[serde(default = "default_name_field")]
    display_name_field: String,
    #[serde(default)]
    allow_registration: bool,
    #[serde(default)]
    auto_link_by_email: bool,
}

#[derive(Serialize)]
struct ProviderTemplate {
    preset: &'static str,
    display_name: &'static str,
    kind: &'static str,
    issuer_url: Option<&'static str>,
    authorization_url: Option<&'static str>,
    token_url: Option<&'static str>,
    userinfo_url: Option<&'static str>,
    emails_url: Option<&'static str>,
    scopes: &'static [&'static str],
}

const TEMPLATES: &[ProviderTemplate] = &[
    ProviderTemplate {
        preset: "github",
        display_name: "GitHub",
        kind: "oauth2",
        issuer_url: None,
        authorization_url: Some("https://github.com/login/oauth/authorize"),
        token_url: Some("https://github.com/login/oauth/access_token"),
        userinfo_url: Some("https://api.github.com/user"),
        emails_url: Some("https://api.github.com/user/emails"),
        scopes: &["read:user", "user:email"],
    },
    ProviderTemplate {
        preset: "gitlab",
        display_name: "GitLab",
        kind: "oauth2",
        issuer_url: None,
        authorization_url: Some("https://gitlab.com/oauth/authorize"),
        token_url: Some("https://gitlab.com/oauth/token"),
        userinfo_url: Some("https://gitlab.com/api/v4/user"),
        emails_url: None,
        scopes: &["read_user"],
    },
    ProviderTemplate {
        preset: "gitee",
        display_name: "Gitee",
        kind: "oauth2",
        issuer_url: None,
        authorization_url: Some("https://gitee.com/oauth/authorize"),
        token_url: Some("https://gitee.com/oauth/token"),
        userinfo_url: Some("https://gitee.com/api/v5/user"),
        emails_url: None,
        scopes: &["user_info"],
    },
    ProviderTemplate {
        preset: "google",
        display_name: "Google",
        kind: "oidc",
        issuer_url: Some("https://accounts.google.com"),
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        emails_url: None,
        scopes: &["openid", "email", "profile"],
    },
    ProviderTemplate {
        preset: "microsoft",
        display_name: "Microsoft",
        kind: "oidc",
        issuer_url: Some("https://login.microsoftonline.com/YOUR_TENANT_ID/v2.0"),
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        emails_url: None,
        scopes: &["openid", "email", "profile"],
    },
    ProviderTemplate {
        preset: "keycloak",
        display_name: "Keycloak",
        kind: "oidc",
        issuer_url: None,
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        emails_url: None,
        scopes: &["openid", "email", "profile"],
    },
    ProviderTemplate {
        preset: "authentik",
        display_name: "Authentik",
        kind: "oidc",
        issuer_url: None,
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        emails_url: None,
        scopes: &["openid", "email", "profile"],
    },
    ProviderTemplate {
        preset: "custom_oauth2",
        display_name: "Custom OAuth2",
        kind: "oauth2",
        issuer_url: None,
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        emails_url: None,
        scopes: &[],
    },
    ProviderTemplate {
        preset: "custom_oidc",
        display_name: "Custom OIDC",
        kind: "oidc",
        issuer_url: None,
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        emails_url: None,
        scopes: &["openid", "email", "profile"],
    },
];

#[derive(Deserialize)]
pub(crate) struct StartQuery {
    invitation: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct FlowPayload {
    pkce_verifier: String,
    nonce: Option<String>,
    invitation: Option<String>,
}

#[derive(Debug)]
struct ExternalClaims {
    subject: String,
    email: Option<String>,
    email_verified: bool,
    display_name: String,
}

pub(crate) async fn list_admin_providers(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    match state.database.list_auth_providers().await {
        Ok(providers) => Json(serde_json::json!({ "providers": providers.into_iter().map(ProviderView::from).collect::<Vec<_>>(), "templates": TEMPLATES })).into_response(),
        Err(error) => { tracing::error!(%error, "failed to list authentication providers"); internal_error_response() }
    }
}

pub(crate) async fn create_provider(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<ProviderRequest>,
) -> Response {
    save_provider(headers, state, 0, request).await
}

pub(crate) async fn update_provider(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<ProviderRequest>,
) -> Response {
    save_provider(headers, state, id, request).await
}

async fn save_provider(
    headers: HeaderMap,
    state: AppState,
    id: i64,
    request: ProviderRequest,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let preserve_secret = request
        .client_secret
        .as_deref()
        .is_none_or(|value| value.trim().is_empty());
    let client_secret = request
        .client_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let provider = AuthProvider {
        id,
        slug: request.slug.trim().to_ascii_lowercase(),
        display_name: request.display_name.trim().to_string(),
        kind: request.kind.trim().to_ascii_lowercase(),
        preset: request.preset.trim().to_ascii_lowercase(),
        enabled: request.enabled,
        client_id: request.client_id.trim().to_string(),
        client_secret,
        issuer_url: clean_optional(request.issuer_url),
        authorization_url: clean_optional(request.authorization_url),
        token_url: clean_optional(request.token_url),
        userinfo_url: clean_optional(request.userinfo_url),
        emails_url: clean_optional(request.emails_url),
        scopes: request
            .scopes
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        subject_field: request.subject_field.trim().to_string(),
        email_field: request.email_field.trim().to_string(),
        email_verified_field: clean_optional(request.email_verified_field),
        display_name_field: request.display_name_field.trim().to_string(),
        allow_registration: request.allow_registration,
        auto_link_by_email: request.auto_link_by_email,
    };
    let has_preserved_secret = if id != 0 && preserve_secret {
        matches!(state.database.auth_provider_by_id(id).await, Ok(Some(existing)) if existing.client_secret.is_some())
    } else {
        false
    };
    if let Err(error) = validate_provider(&provider, has_preserved_secret) {
        return bad_request_response(error.to_string());
    }
    match state
        .database
        .save_auth_provider(&identity.username, &provider, preserve_secret)
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({"id": id}))).into_response(),
        Err(error) if error.to_string().contains("UNIQUE constraint") => {
            conflict("provider slug is already in use")
        }
        Err(error) => {
            tracing::error!(%error, "failed to save authentication provider");
            internal_error_response()
        }
    }
}

pub(crate) async fn delete_provider(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match state.database.auth_provider_identity_count(id).await {
        Ok(0) => {}
        Ok(_) => return conflict("unlink all user identities before deleting this provider"),
        Err(error) => {
            tracing::error!(%error, "failed to inspect authentication provider bindings");
            return internal_error_response();
        }
    }
    match state
        .database
        .delete_auth_provider(&identity.username, id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to delete authentication provider");
            internal_error_response()
        }
    }
}

pub(crate) async fn test_provider(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    let provider = match state.database.auth_provider_by_id(id).await {
        Ok(Some(value)) => value,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load provider for connectivity test");
            return internal_error_response();
        }
    };
    let client = match control_plane_client() {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "failed to build identity HTTP client");
            return internal_error_response();
        }
    };
    let result = if provider.kind == "oidc" {
        match provider.issuer_url.as_deref() {
            Some(issuer) => match IssuerUrl::new(issuer.to_string()) {
                Ok(url) => CoreProviderMetadata::discover_async(url, &client)
                    .await
                    .map(|_| ())
                    .map_err(anyhow::Error::from),
                Err(error) => Err(anyhow!(error)),
            },
            None => Err(anyhow!("issuer URL is missing")),
        }
    } else {
        match provider.authorization_url.as_deref() {
            Some(url) => match client.get(url).send().await {
                Ok(_) => Ok(()),
                Err(error) => Err(error.into()),
            },
            None => Err(anyhow!("authorization URL is missing")),
        }
    };
    match result {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(error) => {
            tracing::warn!(provider = %provider.slug, %error, "authentication provider connectivity test failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": "provider connectivity test failed"})),
            )
                .into_response()
        }
    }
}

pub(crate) async fn public_providers(State(state): State<AppState>) -> Response {
    match state.database.list_auth_providers().await {
        Ok(providers) => Json(
            providers
                .into_iter()
                .filter(|provider| provider.enabled)
                .map(|provider| PublicProvider {
                    slug: provider.slug,
                    display_name: provider.display_name,
                    kind: provider.kind,
                    allow_registration: provider.allow_registration,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to list public authentication providers");
            internal_error_response()
        }
    }
}

pub(crate) async fn start_login(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<StartQuery>,
) -> Response {
    start_flow(&headers, &state, &slug, "login", None, query.invitation).await
}

pub(crate) async fn start_link(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Response {
    let user = match authenticated_user(&headers, &state).await {
        Ok(Some(value)) => value,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "provider link authorization failed");
            return internal_error_response();
        }
    };
    start_flow(&headers, &state, &slug, "link", Some(user.user_id), None).await
}

async fn start_flow(
    headers: &HeaderMap,
    state: &AppState,
    slug: &str,
    mode: &str,
    user_id: Option<i64>,
    invitation: Option<String>,
) -> Response {
    if invitation.as_ref().is_some_and(|token| token.len() > 128) {
        return bad_request_response("invitation token is too long".to_string());
    }
    let provider = match state.database.auth_provider_by_slug(slug).await {
        Ok(Some(value)) if value.enabled => value,
        Ok(_) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load authentication provider");
            return internal_error_response();
        }
    };
    let callback_url = match callback_url(state, headers, &provider.slug) {
        Ok(value) => value,
        Err(error) => return bad_request_response(error.to_string()),
    };
    let state_token = random_token(48);
    let Some(secret) = provider.client_secret.as_deref() else {
        return service_unavailable("external authentication credentials are unavailable");
    };
    let result = if provider.kind == "oidc" {
        oidc_authorization(&provider, secret, &callback_url, &state_token).await
    } else {
        oauth_authorization_with_secret(&provider, secret, &callback_url, &state_token)
    };
    let (authorization_url, verifier, nonce) = match result {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(provider = %provider.slug, %error, "failed to create authentication authorization URL");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    let payload = FlowPayload {
        pkce_verifier: verifier,
        nonce,
        invitation,
    };
    let encoded = match serde_json::to_string(&payload).context("serialize OAuth flow") {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "failed to serialize authentication flow");
            return internal_error_response();
        }
    };
    if let Err(error) = state
        .database
        .store_user_auth_flow(
            &state_token,
            provider.id,
            &encoded,
            mode,
            user_id,
            now() + FLOW_LIFETIME_SECS,
        )
        .await
    {
        tracing::error!(%error, "failed to store authentication flow");
        return internal_error_response();
    }
    let mut response = Redirect::to(authorization_url.as_str()).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        flow_cookie(&state_token, callback_url.starts_with("https://")),
    );
    response
}

pub(crate) async fn callback(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    if query.error.is_some() {
        return callback_error("provider_denied");
    }
    let (Some(code), Some(returned_state)) = (query.code.as_deref(), query.state.as_deref()) else {
        return callback_error("invalid_callback");
    };
    if cookie_value(&headers, FLOW_COOKIE) != Some(returned_state) {
        return callback_error("invalid_state");
    }
    let flow = match state.database.take_user_auth_flow(returned_state).await {
        Ok(Some(value)) => value,
        Ok(None) => return callback_error("expired_state"),
        Err(error) => {
            tracing::error!(%error, "failed to consume authentication flow");
            return callback_error("internal");
        }
    };
    let provider = match state.database.auth_provider_by_id(flow.provider_id).await {
        Ok(Some(value)) if value.enabled && value.slug.eq_ignore_ascii_case(&slug) => value,
        _ => return callback_error("invalid_provider"),
    };
    let payload: FlowPayload =
        match serde_json::from_str(&flow.payload).context("decode OAuth flow") {
            Ok(value) => value,
            Err(error) => {
                tracing::error!(%error, "failed to decode authentication flow");
                return callback_error("invalid_state");
            }
        };
    let callback_url = match callback_url(&state, &headers, &provider.slug) {
        Ok(value) => value,
        Err(_) => return callback_error("invalid_callback"),
    };
    let Some(secret) = provider.client_secret.as_deref() else {
        return callback_error("not_configured");
    };
    let claims = if provider.kind == "oidc" {
        oidc_exchange(&provider, secret, &callback_url, code, &payload).await
    } else {
        oauth_exchange(&provider, secret, &callback_url, code, &payload).await
    };
    let claims = match claims {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(provider = %provider.slug, %error, "external authentication callback failed");
            return callback_error("provider_error");
        }
    };
    complete_identity(
        &state,
        &provider,
        &flow.mode,
        flow.user_id,
        payload.invitation.as_deref(),
        claims,
    )
    .await
}

pub(crate) async fn account_providers(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    let user = match authenticated_user(&headers, &state).await {
        Ok(Some(value)) => value,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "identity list authorization failed");
            return internal_error_response();
        }
    };
    match state.database.list_external_identities(user.user_id).await {
        Ok(identities) => Json(identities).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to list linked identities");
            internal_error_response()
        }
    }
}

pub(crate) async fn unlink_identity(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    let user = match authenticated_user(&headers, &state).await {
        Ok(Some(value)) => value,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "identity unlink authorization failed");
            return internal_error_response();
        }
    };
    let count = match state.database.external_identity_count(user.user_id).await {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "failed to count linked identities");
            return internal_error_response();
        }
    };
    let email_login_available =
        matches!(state.database.smtp_settings().await, Ok(Some(settings)) if settings.enabled);
    if count <= 1 && !email_login_available {
        return conflict(
            "cannot remove the last external identity while email login is unavailable",
        );
    }
    match state
        .database
        .delete_external_identity(&format!("user:{}", user.user_id), user.user_id, id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to unlink external identity");
            internal_error_response()
        }
    }
}

async fn complete_identity(
    state: &AppState,
    provider: &AuthProvider,
    mode: &str,
    flow_user_id: Option<i64>,
    invitation: Option<&str>,
    claims: ExternalClaims,
) -> Response {
    if mode == "link" {
        let Some(user_id) = flow_user_id else {
            return callback_error("invalid_state");
        };
        match state.database.user_account(user_id).await {
            Ok(Some(user)) if !user.disabled => {}
            _ => return callback_error("account_disabled"),
        }
        return match state
            .database
            .bind_external_identity(
                &format!("user:{user_id}"),
                user_id,
                &provider.slug,
                &claims.subject,
                claims.email.as_deref(),
                claims.email_verified,
            )
            .await
        {
            Ok(true) => callback_redirect("/account?provider=linked", None),
            Ok(false) => callback_error("identity_in_use"),
            Err(error) => {
                tracing::error!(%error, "failed to link external identity");
                callback_error("identity_in_use")
            }
        };
    }
    let user = match state
        .database
        .user_by_external_identity(&provider.slug, &claims.subject)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            if !claims.email_verified {
                return callback_error("verified_email_required");
            }
            let Some(email) = claims
                .email
                .as_deref()
                .filter(|value| valid_user_email(value))
            else {
                return callback_error("verified_email_required");
            };
            match state.database.user_by_email(email).await {
                Ok(Some(user)) if provider.auto_link_by_email => {
                    match state
                        .database
                        .bind_external_identity(
                            "oauth:auto-link",
                            user.id,
                            &provider.slug,
                            &claims.subject,
                            Some(email),
                            true,
                        )
                        .await
                    {
                        Ok(true) => user,
                        _ => return callback_error("identity_in_use"),
                    }
                }
                Ok(Some(_)) => return callback_error("manual_link_required"),
                Ok(None) => {
                    let invitation_id =
                        match registration_allowed(state, provider, email, invitation).await {
                            Ok(value) => value,
                            Err(error) => return callback_error(error),
                        };
                    let display_name = if claims.display_name.trim().is_empty() {
                        email.split('@').next().unwrap_or("user")
                    } else {
                        claims.display_name.trim()
                    };
                    let user = match state
                        .database
                        .create_user_with_external_identity(ExternalRegistration {
                            actor: &format!("oauth:{}", provider.slug),
                            email,
                            display_name,
                            routing_min_length: state.config().user_access.routing_id_min_length,
                            provider_slug: &provider.slug,
                            provider_subject: &claims.subject,
                            invitation_id,
                        })
                        .await
                    {
                        Ok(Some(value)) => value,
                        Ok(None) => return callback_error("manual_link_required"),
                        Err(error) => {
                            tracing::error!(%error, "failed to create OAuth user");
                            return callback_error("internal");
                        }
                    };
                    user
                }
                Err(error) => {
                    tracing::error!(%error, "failed to find OAuth user by email");
                    return callback_error("internal");
                }
            }
        }
        Err(error) => {
            tracing::error!(%error, "failed to resolve external identity");
            return callback_error("internal");
        }
    };
    let session = match state
        .database
        .create_user_session(user.id, &format!("oauth:{}", provider.slug))
        .await
    {
        Ok(Some(value)) => value,
        _ => return callback_error("account_disabled"),
    };
    if let Err(error) = state
        .database
        .audit_user_login(user.id, &format!("oauth:{}", provider.slug))
        .await
    {
        tracing::error!(%error, "failed to audit external user login");
    }
    callback_redirect("/account", Some(&session.token))
}

async fn registration_allowed(
    state: &AppState,
    provider: &AuthProvider,
    email: &str,
    invitation: Option<&str>,
) -> Result<Option<i64>, &'static str> {
    if !provider.allow_registration {
        return Err("registration_disabled");
    }
    let config = state.config();
    match config.registration.mode.as_str() {
        "open" => Ok(None),
        "domain_allowlist"
            if email_domain_allowed(email, &config.registration.allowed_email_domains) =>
        {
            Ok(None)
        }
        "invite_only" => {
            let Some(token) = invitation else {
                return Err("invitation_required");
            };
            state
                .database
                .valid_invitation(email, token)
                .await
                .map_err(|_| "internal")?
                .ok_or("invalid_invitation")
                .map(Some)
        }
        _ => Err("registration_disabled"),
    }
}

fn oauth_authorization_with_secret(
    provider: &AuthProvider,
    secret: &str,
    callback: &str,
    state: &str,
) -> anyhow::Result<(Url, String, Option<String>)> {
    let client = oauth_client(provider, secret, callback)?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut request = client
        .authorize_url(|| CsrfToken::new(state.to_string()))
        .set_pkce_challenge(challenge);
    for scope in &provider.scopes {
        request = request.add_scope(Scope::new(scope.clone()));
    }
    let (url, _) = request.url();
    Ok((url, verifier.secret().to_string(), None))
}

async fn oauth_exchange(
    provider: &AuthProvider,
    secret: &str,
    callback: &str,
    code: &str,
    payload: &FlowPayload,
) -> anyhow::Result<ExternalClaims> {
    let client = oauth_client(provider, secret, callback)?;
    let http = control_plane_client()?;
    let token = client
        .exchange_code(AuthorizationCode::new(code.to_string()))
        .set_pkce_verifier(PkceCodeVerifier::new(payload.pkce_verifier.clone()))
        .request_async(&http)
        .await
        .context("OAuth token exchange failed")?;
    let userinfo_url = provider
        .userinfo_url
        .as_deref()
        .context("userinfo URL is missing")?;
    let value: Value = http
        .get(userinfo_url)
        .bearer_auth(token.access_token().secret())
        .header(header::USER_AGENT, "MirrorProxy")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let subject =
        json_text(&value, &provider.subject_field).context("provider subject is missing")?;
    let mut email =
        json_text(&value, &provider.email_field).map(|value| value.to_ascii_lowercase());
    let mut verified = provider
        .email_verified_field
        .as_deref()
        .and_then(|field| json_bool(&value, field))
        .unwrap_or(false);
    if (!verified || email.is_none()) && provider.emails_url.is_some() {
        let emails: Value = http
            .get(provider.emails_url.as_deref().unwrap())
            .bearer_auth(token.access_token().secret())
            .header(header::USER_AGENT, "MirrorProxy")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(items) = emails.as_array() {
            if let Some(item) = items
                .iter()
                .find(|item| {
                    item.get("verified").and_then(Value::as_bool) == Some(true)
                        && item.get("primary").and_then(Value::as_bool) == Some(true)
                })
                .or_else(|| {
                    items
                        .iter()
                        .find(|item| item.get("verified").and_then(Value::as_bool) == Some(true))
                })
            {
                email = item
                    .get("email")
                    .and_then(Value::as_str)
                    .map(str::to_ascii_lowercase);
                verified = email.is_some();
            }
        }
    }
    let display_name =
        json_text(&value, &provider.display_name_field).unwrap_or_else(|| subject.clone());
    Ok(ExternalClaims {
        subject,
        email,
        email_verified: verified,
        display_name,
    })
}

async fn oidc_authorization(
    provider: &AuthProvider,
    secret: &str,
    callback: &str,
    state: &str,
) -> anyhow::Result<(Url, String, Option<String>)> {
    let http = control_plane_client()?;
    let client = oidc_client(provider, secret, callback, &http).await?;
    let (challenge, verifier) = OidcPkceCodeChallenge::new_random_sha256();
    let nonce = Nonce::new_random();
    let nonce_value = nonce.secret().to_string();
    let state = state.to_string();
    let mut request = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            move || OidcCsrfToken::new(state),
            || nonce,
        )
        .set_pkce_challenge(challenge);
    for scope in &provider.scopes {
        request = request.add_scope(OidcScope::new(scope.clone()));
    }
    let (url, _, _) = request.url();
    Ok((url, verifier.secret().to_string(), Some(nonce_value)))
}

async fn oidc_exchange(
    provider: &AuthProvider,
    secret: &str,
    callback: &str,
    code: &str,
    payload: &FlowPayload,
) -> anyhow::Result<ExternalClaims> {
    let http = control_plane_client()?;
    let client = oidc_client(provider, secret, callback, &http).await?;
    let token = client
        .exchange_code(openidconnect::AuthorizationCode::new(code.to_string()))?
        .set_pkce_verifier(OidcPkceCodeVerifier::new(payload.pkce_verifier.clone()))
        .request_async(&http)
        .await
        .context("OIDC token exchange failed")?;
    let id_token = token
        .id_token()
        .context("OIDC token response did not contain an ID token")?;
    let verifier = client.id_token_verifier();
    let nonce = Nonce::new(payload.nonce.clone().context("OIDC nonce is missing")?);
    let claims = id_token
        .claims(&verifier, &nonce)
        .context("OIDC ID token validation failed")?;
    if let Some(expected) = claims.access_token_hash() {
        let actual = AccessTokenHash::from_token(
            token.access_token(),
            id_token.signing_alg()?,
            id_token.signing_key(&verifier)?,
        )?;
        if actual != *expected {
            return Err(anyhow!("OIDC access token hash mismatch"));
        }
    }
    let subject = claims.subject().as_str().to_string();
    let mut email = claims
        .email()
        .map(|value| value.as_str().to_ascii_lowercase());
    let mut email_verified = claims.email_verified() == Some(true);
    let mut display_name = claims
        .name()
        .and_then(|name| name.get(None))
        .map(|name| name.as_str().to_string());
    if (!email_verified || email.is_none()) && client.user_info_url().is_some() {
        let userinfo: CoreUserInfoClaims = client
            .user_info(token.access_token().clone(), Some(claims.subject().clone()))?
            .request_async(&http)
            .await
            .context("OIDC UserInfo request failed")?;
        if userinfo.email_verified() == Some(true) {
            email = userinfo
                .email()
                .map(|value| value.as_str().to_ascii_lowercase());
            email_verified = email.is_some();
        }
        display_name = display_name.or_else(|| {
            userinfo
                .name()
                .and_then(|name| name.get(None))
                .map(|name| name.as_str().to_string())
        });
    }
    let display_name = display_name.unwrap_or_else(|| {
        email
            .as_deref()
            .and_then(|value| value.split('@').next())
            .unwrap_or(&subject)
            .to_string()
    });
    Ok(ExternalClaims {
        subject,
        email,
        email_verified,
        display_name,
    })
}

type OAuthClient = BasicClient<
    oauth2::EndpointSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointSet,
>;

fn oauth_client(
    provider: &AuthProvider,
    secret: &str,
    callback: &str,
) -> anyhow::Result<OAuthClient> {
    let client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(secret.to_string()))
        .set_auth_uri(AuthUrl::new(
            provider
                .authorization_url
                .clone()
                .context("authorization URL is missing")?,
        )?)
        .set_token_uri(TokenUrl::new(
            provider.token_url.clone().context("token URL is missing")?,
        )?)
        .set_redirect_uri(RedirectUrl::new(callback.to_string())?);
    Ok(
        if matches!(provider.preset.as_str(), "github" | "gitlab" | "gitee") {
            client.set_auth_type(AuthType::RequestBody)
        } else {
            client
        },
    )
}

async fn oidc_client(
    provider: &AuthProvider,
    secret: &str,
    callback: &str,
    http: &reqwest::Client,
) -> anyhow::Result<
    CoreClient<
        openidconnect::EndpointSet,
        openidconnect::EndpointNotSet,
        openidconnect::EndpointNotSet,
        openidconnect::EndpointNotSet,
        openidconnect::EndpointMaybeSet,
        openidconnect::EndpointMaybeSet,
    >,
> {
    let metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new(
            provider
                .issuer_url
                .clone()
                .context("issuer URL is missing")?,
        )?,
        http,
    )
    .await?;
    Ok(CoreClient::from_provider_metadata(
        metadata,
        OidcClientId::new(provider.client_id.clone()),
        Some(OidcClientSecret::new(secret.to_string())),
    )
    .set_redirect_uri(OidcRedirectUrl::new(callback.to_string())?))
}

fn control_plane_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .context("build identity HTTP client")
}

fn callback_url(state: &AppState, headers: &HeaderMap, slug: &str) -> anyhow::Result<String> {
    let base = state.public_base_url(headers);
    if base.is_empty() {
        anyhow::bail!("public_base_url or forwarded Host is required for external authentication");
    }
    Ok(format!(
        "{}/api/auth/{}/callback",
        base.trim_end_matches('/'),
        slug
    ))
}

fn validate_provider(provider: &AuthProvider, has_preserved_secret: bool) -> anyhow::Result<()> {
    if !(2..=50).contains(&provider.slug.len())
        || !provider.slug.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        anyhow::bail!("slug must contain 2 to 50 lowercase ASCII letters, numbers, or hyphens");
    }
    if provider.display_name.is_empty()
        || provider.display_name.chars().count() > 80
        || provider.client_id.is_empty()
    {
        anyhow::bail!("display_name and client_id are required");
    }
    if provider.kind != "oauth2" && provider.kind != "oidc" {
        anyhow::bail!("kind must be oauth2 or oidc");
    }
    if provider.enabled && provider.client_secret.is_none() && !has_preserved_secret {
        anyhow::bail!("client_secret is required before enabling a provider");
    }
    if provider.kind == "oidc" {
        validate_https(
            provider
                .issuer_url
                .as_deref()
                .context("issuer_url is required for OIDC")?,
        )?;
        if !provider.scopes.iter().any(|scope| scope == "openid") {
            anyhow::bail!("OIDC scopes must include openid");
        }
    } else {
        for value in [
            provider.authorization_url.as_deref(),
            provider.token_url.as_deref(),
            provider.userinfo_url.as_deref(),
        ] {
            validate_https(value.context(
                "authorization_url, token_url, and userinfo_url are required for OAuth2",
            )?)?;
        }
        if let Some(value) = provider.emails_url.as_deref() {
            validate_https(value)?;
        }
        if provider.subject_field.is_empty() {
            anyhow::bail!("subject_field is required for OAuth2");
        }
    }
    Ok(())
}

fn validate_https(value: &str) -> anyhow::Result<()> {
    let url = Url::parse(value)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        anyhow::bail!("provider endpoints must be credential-free HTTPS URLs without fragments");
    }
    Ok(())
}

fn callback_redirect(path: &str, session: Option<&str>) -> Response {
    let mut response = Redirect::to(path).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        session
            .map(user_session_cookie)
            .unwrap_or_else(clear_flow_cookie),
    );
    if session.is_some() {
        response
            .headers_mut()
            .append(header::SET_COOKIE, clear_flow_cookie());
    }
    response
}

fn callback_error(code: &str) -> Response {
    callback_redirect(&format!("/login?oauth_error={code}"), None)
}
fn flow_cookie(value: &str, secure: bool) -> HeaderValue {
    HeaderValue::from_str(&format!("{FLOW_COOKIE}={value}; Path=/api/auth; HttpOnly; SameSite=Lax; Max-Age={FLOW_LIFETIME_SECS}{}", if secure { "; Secure" } else { "" })).expect("generated OAuth flow cookie is valid")
}
fn clear_flow_cookie() -> HeaderValue {
    HeaderValue::from_static(
        "mirrorproxy_oauth_state=; Path=/api/auth; HttpOnly; SameSite=Lax; Max-Age=0",
    )
}
fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|item| item.trim().split_once('='))
        .find_map(|(cookie_name, value)| {
            (cookie_name == name && !value.is_empty()).then_some(value)
        })
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
        .unwrap_or_default()
        .as_secs() as i64
}
fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
fn default_preset() -> String {
    "custom_oauth2".to_string()
}
fn default_subject_field() -> String {
    "id".to_string()
}
fn default_email_field() -> String {
    "email".to_string()
}
fn default_name_field() -> String {
    "name".to_string()
}
fn json_value<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(value, |current, key| current.get(key))
}
fn json_text(value: &Value, path: &str) -> Option<String> {
    let value = json_value(value, path)?;
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|number| number.to_string()))
        .or_else(|| value.as_u64().map(|number| number.to_string()))
}
fn json_bool(value: &Value, path: &str) -> Option<bool> {
    json_value(value, path).and_then(Value::as_bool)
}
fn email_domain_allowed(email: &str, domains: &[String]) -> bool {
    email
        .rsplit_once('@')
        .is_some_and(|(_, domain)| domains.iter().any(|allowed| allowed == domain))
}
fn conflict(message: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({"error": message})),
    )
        .into_response()
}
fn service_unavailable(message: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error": message})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_cover_common_oauth_and_oidc_providers() {
        for preset in [
            "github",
            "gitlab",
            "gitee",
            "google",
            "microsoft",
            "keycloak",
            "authentik",
            "custom_oauth2",
            "custom_oidc",
        ] {
            assert!(TEMPLATES.iter().any(|template| template.preset == preset));
        }
    }

    #[test]
    fn oauth_authorization_uses_state_and_pkce() {
        let provider = AuthProvider {
            id: 1,
            slug: "github".into(),
            display_name: "GitHub".into(),
            kind: "oauth2".into(),
            preset: "github".into(),
            enabled: true,
            client_id: "client".into(),
            client_secret: Some("unused".into()),
            issuer_url: None,
            authorization_url: Some("https://github.com/login/oauth/authorize".into()),
            token_url: Some("https://github.com/login/oauth/access_token".into()),
            userinfo_url: Some("https://api.github.com/user".into()),
            emails_url: None,
            scopes: vec!["user:email".into()],
            subject_field: "id".into(),
            email_field: "email".into(),
            email_verified_field: None,
            display_name_field: "name".into(),
            allow_registration: false,
            auto_link_by_email: false,
        };
        let (url, verifier, _) = oauth_authorization_with_secret(
            &provider,
            "secret",
            "https://proxy.example/api/auth/github/callback",
            "expected-state",
        )
        .unwrap();
        let parameters = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(parameters.get("state").unwrap(), "expected-state");
        assert_eq!(parameters.get("code_challenge_method").unwrap(), "S256");
        assert!(!verifier.is_empty());
    }

    #[test]
    fn provider_api_never_serializes_the_client_secret() {
        let provider = AuthProvider {
            id: 1,
            slug: "github".into(),
            display_name: "GitHub".into(),
            kind: "oauth2".into(),
            preset: "github".into(),
            enabled: true,
            client_id: "client".into(),
            client_secret: Some("plain-secret".into()),
            issuer_url: None,
            authorization_url: Some("https://github.com/login/oauth/authorize".into()),
            token_url: Some("https://github.com/login/oauth/access_token".into()),
            userinfo_url: Some("https://api.github.com/user".into()),
            emails_url: None,
            scopes: vec!["user:email".into()],
            subject_field: "id".into(),
            email_field: "email".into(),
            email_verified_field: None,
            display_name_field: "name".into(),
            allow_registration: false,
            auto_link_by_email: false,
        };
        let json = serde_json::to_string(&ProviderView::from(provider)).unwrap();
        assert!(json.contains("\"has_client_secret\":true"));
        assert!(!json.contains("plain-secret"));
    }

    #[test]
    fn rejects_insecure_provider_endpoints_and_oidc_without_openid_scope() {
        assert!(validate_https("http://identity.example/token").is_err());
        assert!(validate_https("https://user:password@identity.example/token").is_err());
        let mut provider = AuthProvider {
            id: 0,
            slug: "company".into(),
            display_name: "Company".into(),
            kind: "oidc".into(),
            preset: "custom_oidc".into(),
            enabled: false,
            client_id: "client".into(),
            client_secret: None,
            issuer_url: Some("https://identity.example".into()),
            authorization_url: None,
            token_url: None,
            userinfo_url: None,
            emails_url: None,
            scopes: vec!["email".into()],
            subject_field: "id".into(),
            email_field: "email".into(),
            email_verified_field: None,
            display_name_field: "name".into(),
            allow_registration: false,
            auto_link_by_email: false,
        };
        assert!(validate_provider(&provider, false).is_err());
        provider.scopes.push("openid".into());
        assert!(validate_provider(&provider, false).is_ok());
    }
}
