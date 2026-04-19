use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::HttpClient;
use crate::error::PlankaError;
use crate::models::User;
use crate::transport::TransportPolicy;

/// Request body for `POST /api/access-tokens`.
#[derive(Serialize)]
struct LoginRequest<'a> {
    email: &'a str,
    password: &'a str,
}

/// Response from `POST /api/access-tokens`.
#[derive(Deserialize)]
struct LoginResponse {
    item: String,
}

/// Response from `GET /api/users/me`.
#[derive(Deserialize)]
struct UserMeResponse {
    item: User,
}

/// Exchange email + password for an API token.
///
/// # Errors
/// Returns `PlankaError::AuthenticationFailed` on invalid credentials.
pub async fn login(server: &Url, email: &str, password: &str) -> Result<String, PlankaError> {
    login_with_policy(server, email, password, TransportPolicy::default()).await
}

/// Exchange email + password for an API token using an explicit transport policy.
///
/// # Errors
/// Returns `PlankaError::AuthenticationFailed` on invalid credentials.
pub async fn login_with_policy(
    server: &Url,
    email: &str,
    password: &str,
    policy: TransportPolicy,
) -> Result<String, PlankaError> {
    let client = HttpClient::unauthenticated_with_policy(server.clone(), policy)?;
    let body = LoginRequest { email, password };

    match client
        .post::<_, LoginResponse>("/api/access-tokens", &body)
        .await
    {
        Ok(login_resp) => Ok(login_resp.item),
        Err(PlankaError::AuthenticationFailed { .. }) => Err(PlankaError::AuthenticationFailed {
            message: "Invalid email or password.".to_string(),
        }),
        Err(err) => Err(err),
    }
}

/// Validate a token by hitting `GET /api/users/me`.
///
/// # Errors
/// Returns `PlankaError::AuthenticationFailed` if the token is invalid.
pub async fn validate_token(server: &Url, token: &str) -> Result<User, PlankaError> {
    validate_token_with_policy(server, token, TransportPolicy::default()).await
}

/// Validate a token by hitting `GET /api/users/me` using an explicit transport policy.
///
/// # Errors
/// Returns `PlankaError::AuthenticationFailed` if the token is invalid.
pub async fn validate_token_with_policy(
    server: &Url,
    token: &str,
    policy: TransportPolicy,
) -> Result<User, PlankaError> {
    let client = HttpClient::with_policy(server.clone(), token, policy)?;
    let resp: UserMeResponse = client.get("/api/users/me").await?;
    Ok(resp.item)
}
