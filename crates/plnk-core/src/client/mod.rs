use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::{debug, trace};
use url::Url;

use crate::error::PlankaError;

/// HTTP transport layer for the Planka API.
///
/// Thin wrapper around `reqwest::Client` that handles base URL construction,
/// auth header injection, request/response logging, and HTTP error mapping.
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: Client,
    base_url: Url,
}

impl HttpClient {
    /// Create a new HTTP client for the given Planka server.
    ///
    /// # Errors
    /// Returns `PlankaError` if the reqwest client cannot be built.
    pub fn new(base_url: Url, api_key: &str) -> Result<Self, PlankaError> {
        let mut headers = HeaderMap::new();

        let mut auth_value = HeaderValue::from_str(api_key).map_err(|e| PlankaError::ApiError {
            status: 0,
            message: format!("Invalid API key format: {e}"),
        })?;
        auth_value.set_sensitive(true);
        headers.insert("X-API-Key", auth_value);

        let inner = Client::builder()
            .default_headers(headers)
            .user_agent(format!("plnk/{}", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PlankaError::ApiError {
                status: 0,
                message: format!("Failed to build HTTP client: {e}"),
            })?;

        Ok(Self { inner, base_url })
    }

    /// Build a full URL from a path.
    fn url(&self, path: &str) -> Result<Url, PlankaError> {
        self.base_url.join(path).map_err(PlankaError::from)
    }

    /// GET a resource, deserializing the response body.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, PlankaError> {
        let url = self.url(path)?;
        debug!("GET {url}");

        let resp = self.inner.get(url.clone()).send().await?;
        let status = resp.status();
        debug!("{status} {url}");

        if !status.is_success() {
            return Err(Self::map_error(
                "GET",
                path,
                status,
                &resp.text().await.unwrap_or_default(),
            ));
        }

        let text = resp.text().await?;
        trace!("Response body: {text}");
        serde_json::from_str(&text).map_err(PlankaError::from)
    }

    /// POST with a JSON body, deserializing the response.
    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, PlankaError> {
        let url = self.url(path)?;
        debug!("POST {url}");
        trace!(
            "Request body: {}",
            serde_json::to_string(body).unwrap_or_default()
        );

        let resp = self.inner.post(url.clone()).json(body).send().await?;
        let status = resp.status();
        debug!("{status} {url}");

        if !status.is_success() {
            return Err(Self::map_error(
                "POST",
                path,
                status,
                &resp.text().await.unwrap_or_default(),
            ));
        }

        let text = resp.text().await?;
        trace!("Response body: {text}");
        serde_json::from_str(&text).map_err(PlankaError::from)
    }

    /// PATCH with a JSON body, deserializing the response.
    pub async fn patch<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, PlankaError> {
        let url = self.url(path)?;
        debug!("PATCH {url}");
        trace!(
            "Request body: {}",
            serde_json::to_string(body).unwrap_or_default()
        );

        let resp = self.inner.patch(url.clone()).json(body).send().await?;
        let status = resp.status();
        debug!("{status} {url}");

        if !status.is_success() {
            return Err(Self::map_error(
                "PATCH",
                path,
                status,
                &resp.text().await.unwrap_or_default(),
            ));
        }

        let text = resp.text().await?;
        trace!("Response body: {text}");
        serde_json::from_str(&text).map_err(PlankaError::from)
    }

    /// DELETE a resource. Returns `()` on success.
    pub async fn delete(&self, path: &str) -> Result<(), PlankaError> {
        let url = self.url(path)?;
        debug!("DELETE {url}");

        let resp = self.inner.delete(url.clone()).send().await?;
        let status = resp.status();
        debug!("{status} {url}");

        if !status.is_success() {
            return Err(Self::map_error(
                "DELETE",
                path,
                status,
                &resp.text().await.unwrap_or_default(),
            ));
        }

        Ok(())
    }

    /// GET raw bytes (for file downloads).
    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>, PlankaError> {
        let url = self.url(path)?;
        debug!("GET (bytes) {url}");

        let resp = self.inner.get(url.clone()).send().await?;
        let status = resp.status();
        debug!("{status} {url}");

        if !status.is_success() {
            return Err(Self::map_error(
                "GET",
                path,
                status,
                &resp.text().await.unwrap_or_default(),
            ));
        }

        Ok(resp.bytes().await?.to_vec())
    }

    /// POST a multipart form (for file uploads).
    pub async fn post_multipart<T: DeserializeOwned>(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> Result<T, PlankaError> {
        let url = self.url(path)?;
        debug!("POST (multipart) {url}");

        let resp = self.inner.post(url.clone()).multipart(form).send().await?;
        let status = resp.status();
        debug!("{status} {url}");

        if !status.is_success() {
            return Err(Self::map_error(
                "POST",
                path,
                status,
                &resp.text().await.unwrap_or_default(),
            ));
        }

        let text = resp.text().await?;
        trace!("Response body: {text}");
        serde_json::from_str(&text).map_err(PlankaError::from)
    }

    /// Map HTTP status codes to typed `PlankaError` variants.
    fn map_error(method: &str, path: &str, status: StatusCode, body: &str) -> PlankaError {
        let message = if body.is_empty() {
            status
                .canonical_reason()
                .unwrap_or("Unknown error")
                .to_string()
        } else {
            // Try to extract message from JSON error response
            serde_json::from_str::<serde_json::Value>(body)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
                .unwrap_or_else(|| body.to_string())
        };

        match status {
            StatusCode::UNAUTHORIZED => PlankaError::AuthenticationFailed { message },
            StatusCode::NOT_FOUND => PlankaError::Remote404 {
                method: method.to_string(),
                path: path.to_string(),
                server_message: message,
            },
            _ => PlankaError::ApiError {
                status: status.as_u16(),
                message,
            },
        }
    }
}
