use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::HttpClient;
use crate::error::PlankaError;
use crate::models::User;

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
    // Build a temporary client without auth for the login request
    let client = reqwest::Client::builder()
        .user_agent(format!("plnk/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| PlankaError::ApiError {
            status: 0,
            message: format!("Failed to build HTTP client: {e}"),
        })?;

    let url = server.join("/api/access-tokens")?;
    let body = LoginRequest { email, password };

    let resp = client.post(url).json(&body).send().await?;
    let status = resp.status();

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(PlankaError::AuthenticationFailed {
            message: "Invalid email or password.".to_string(),
        });
    }

    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(PlankaError::ApiError {
            status: status.as_u16(),
            message: text,
        });
    }

    let login_resp: LoginResponse = resp.json().await?;
    Ok(login_resp.item)
}

/// Validate a token by hitting `GET /api/users/me`.
///
/// # Errors
/// Returns `PlankaError::AuthenticationFailed` if the token is invalid.
pub async fn validate_token(server: &Url, token: &str) -> Result<User, PlankaError> {
    let client = HttpClient::new(server.clone(), token)?;
    let resp: UserMeResponse = client.get("/api/users/me").await?;
    Ok(resp.item)
}
