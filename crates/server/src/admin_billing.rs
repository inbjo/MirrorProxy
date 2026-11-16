use super::*;

#[derive(Deserialize)]
pub struct UserSummaryQuery {
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default = "default_page_size")]
    per_page: u32,
    #[serde(default)]
    search: String,
}

pub async fn user_summaries(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<UserSummaryQuery>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    if query.search.len() > 512 {
        return bad_request_response("search is too long".into());
    }
    let (_, month) = quota_period(&state.config().quota.timezone);
    match state
        .database
        .user_summaries(
            query.search.trim(),
            query.page.max(1),
            query.per_page.clamp(1, 100),
            &month,
        )
        .await
    {
        Ok(page) => Json(page).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load user summaries");
            internal_error_response()
        }
    }
}

#[derive(Deserialize)]
pub(super) struct BillingGroupRequest {
    name: String,
    monthly_gb: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct GroupTargetAccessRequest {
    target_codes: Vec<String>,
}

pub(super) async fn list_billing_groups(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    match state.database.list_billing_groups().await {
        Ok(groups) => Json(groups).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to list billing groups");
            internal_error_response()
        }
    }
}

pub(super) async fn create_billing_group(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<BillingGroupRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return bad_request_response("group name must contain 1 to 80 characters".to_string());
    }
    let limit = match quota_gb_to_bytes(request.monthly_gb) {
        Ok(limit) => limit,
        Err(message) => return bad_request_response(message.to_string()),
    };
    match state
        .database
        .create_billing_group(&identity.username, name, limit)
        .await
    {
        Ok(Some(group)) => (StatusCode::CREATED, Json(group)).into_response(),
        Ok(None) => conflict_response("billing group name already exists"),
        Err(error) => {
            tracing::error!(%error, "failed to create billing group");
            internal_error_response()
        }
    }
}

pub(super) async fn update_billing_group(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    Json(request): Json<BillingGroupRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return bad_request_response("group name must contain 1 to 80 characters".to_string());
    }
    let limit = match quota_gb_to_bytes(request.monthly_gb) {
        Ok(limit) => limit,
        Err(message) => return bad_request_response(message.to_string()),
    };
    match state
        .database
        .update_billing_group(&identity.username, id, name, limit)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "billing group not found"})),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to update billing group");
            internal_error_response()
        }
    }
}

pub(super) async fn admin_group_target_access(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    match state.database.group_target_access(id).await {
        Ok(Some(policy)) => Json(policy).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load team target access");
            internal_error_response()
        }
    }
}

pub(super) async fn update_group_target_access(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    Json(request): Json<GroupTargetAccessRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let mut supported = Config::default().enabled_proxies;
    supported.push("doh".to_string());
    let mut target_codes = request
        .target_codes
        .into_iter()
        .map(|target| target.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    target_codes.sort();
    target_codes.dedup();
    if target_codes
        .iter()
        .any(|target| !supported.contains(target))
    {
        return bad_request_response(
            "target_codes contains an unsupported proxy target".to_string(),
        );
    }
    match state
        .database
        .set_group_target_access(&identity.username, id, &target_codes)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to update team target access");
            internal_error_response()
        }
    }
}

#[derive(Deserialize)]
pub(super) struct UserBillingRequest {
    group_id: Option<i64>,
    quota_mode: String,
    monthly_gb: Option<u64>,
}

pub(super) async fn admin_user_billing(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    match state.database.user_billing_profile(id).await {
        Ok(Some(profile)) => Json(profile).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "user not found"})),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load user billing profile");
            internal_error_response()
        }
    }
}

pub(super) async fn update_user_billing(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    Json(request): Json<UserBillingRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if !matches!(
        request.quota_mode.as_str(),
        "default" | "unlimited" | "custom"
    ) || (request.quota_mode == "custom" && request.monthly_gb.is_none())
        || (request.quota_mode != "custom" && request.monthly_gb.is_some())
    {
        return bad_request_response("quota_mode and monthly_gb are inconsistent".to_string());
    }
    let limit = match quota_gb_to_bytes(request.monthly_gb) {
        Ok(limit) => limit,
        Err(message) => return bad_request_response(message.to_string()),
    };
    match state
        .database
        .set_user_billing_profile(
            &identity.username,
            id,
            request.group_id,
            &request.quota_mode,
            limit,
        )
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "user or billing group not found"})),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to update user billing profile");
            internal_error_response()
        }
    }
}
