use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::{debug, trace};
use url::Url;

use crate::error::PlankaError;
use crate::transport::{TransportPolicy, TransportRuntime};

/// HTTP transport layer for the Planka API.
///
/// Thin wrapper around `reqwest::Client` that handles base URL construction,
/// auth header injection, request/response logging, and HTTP error mapping.
/// All outbound requests flow through a shared `TransportRuntime`, which
/// enforces retries, rate limiting, and concurrency limits.
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: Client,
    base_url: Url,
    transport: TransportRuntime,
}

impl HttpClient {
    /// Create a new authenticated HTTP client for the given Planka server.
    ///
    /// # Errors
    /// Returns `PlankaError` if the reqwest client cannot be built.
    pub fn new(base_url: Url, api_key: &str) -> Result<Self, PlankaError> {
        Self::with_policy(base_url, api_key, TransportPolicy::default())
    }

    /// Create a new authenticated HTTP client with an explicit transport policy.
    ///
    /// # Errors
    /// Returns `PlankaError` if the policy is invalid or the reqwest client
    /// cannot be built.
    pub fn with_policy(
        base_url: Url,
        api_key: &str,
        policy: TransportPolicy,
    ) -> Result<Self, PlankaError> {
        Self::build(base_url, Some(api_key), policy)
    }

    /// Create a new unauthenticated HTTP client with the default transport policy.
    ///
    /// Used for login/bootstrap flows that do not yet have an API token.
    ///
    /// # Errors
    /// Returns `PlankaError` if the reqwest client cannot be built.
    pub fn unauthenticated(base_url: Url) -> Result<Self, PlankaError> {
        Self::unauthenticated_with_policy(base_url, TransportPolicy::default())
    }

    /// Create a new unauthenticated HTTP client with an explicit transport policy.
    ///
    /// # Errors
    /// Returns `PlankaError` if the policy is invalid or the reqwest client
    /// cannot be built.
    pub fn unauthenticated_with_policy(
        base_url: Url,
        policy: TransportPolicy,
    ) -> Result<Self, PlankaError> {
        Self::build(base_url, None, policy)
    }

    /// Access the shared transport policy for this client.
    #[must_use]
    pub fn transport_policy(&self) -> &TransportPolicy {
        self.transport.policy()
    }

    fn build(
        base_url: Url,
        api_key: Option<&str>,
        policy: TransportPolicy,
    ) -> Result<Self, PlankaError> {
        let mut headers = HeaderMap::new();

        if let Some(api_key) = api_key {
            let mut auth_value =
                HeaderValue::from_str(api_key).map_err(|e| PlankaError::ApiError {
                    status: 0,
                    message: format!("Invalid API key format: {e}"),
                })?;
            auth_value.set_sensitive(true);
            headers.insert("X-API-Key", auth_value);
        }

        let inner = Client::builder()
            .default_headers(headers)
            .user_agent(format!("plnk/{}", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PlankaError::ApiError {
                status: 0,
                message: format!("Failed to build HTTP client: {e}"),
            })?;

        let transport = TransportRuntime::new(policy)?;

        Ok(Self {
            inner,
            base_url,
            transport,
        })
    }

    /// Build a full URL from a path.
    fn url(&self, path: &str) -> Result<Url, PlankaError> {
        self.base_url.join(path).map_err(PlankaError::from)
    }

    async fn send(
        &self,
        method: &str,
        path: &str,
        request: RequestBuilder,
    ) -> Result<reqwest::Response, PlankaError> {
        let retry_attempts = self.transport.policy().retry_attempts;
        let retryable_method =
            retry_attempts > 0 && self.transport.retries_allowed_for_method(method);

        if !retryable_method {
            let send_result = {
                let _guard = self.transport.acquire().await?;
                request.send().await
            };
            return send_result.map_err(PlankaError::from);
        }

        let template = request.try_clone().ok_or_else(|| PlankaError::ApiError {
            status: 0,
            message: format!("Unable to clone {method} request for retries: {path}"),
        })?;

        let mut retry_number = 0;
        loop {
            let current_request = template.try_clone().ok_or_else(|| PlankaError::ApiError {
                status: 0,
                message: format!("Unable to clone {method} request for retries: {path}"),
            })?;

            let send_result = {
                let _guard = self.transport.acquire().await?;
                current_request.send().await
            };
            match send_result {
                Ok(response) => {
                    let status = response.status();
                    if retry_number < retry_attempts
                        && self.transport.should_retry_status(method, status)
                    {
                        retry_number += 1;
                        let retry_after_delay =
                            self.transport.retry_delay_from_headers(response.headers());
                        let delay = retry_after_delay.unwrap_or_else(|| {
                            self.transport.retry_delay_for_attempt(retry_number)
                        });
                        let source = if retry_after_delay.is_some() {
                            "retry-after"
                        } else {
                            "backoff"
                        };
                        self.transport
                            .sleep_before_retry(method, path, retry_number, delay, source)
                            .await;
                        continue;
                    }

                    return Ok(response);
                }
                Err(error) => {
                    if retry_number < retry_attempts
                        && self.transport.should_retry_error(method, &error)
                    {
                        retry_number += 1;
                        let delay = self.transport.retry_delay_for_attempt(retry_number);
                        self.transport
                            .sleep_before_retry(method, path, retry_number, delay, "transport")
                            .await;
                        continue;
                    }

                    return Err(PlankaError::from(error));
                }
            }
        }
    }

    async fn decode_json<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        response: reqwest::Response,
        url: &Url,
    ) -> Result<T, PlankaError> {
        let status = response.status();
        debug!("{status} {url}");

        if !status.is_success() {
            return Err(Self::map_error(
                method,
                path,
                status,
                &response.text().await.unwrap_or_default(),
            ));
        }

        let text = response.text().await?;
        trace!("Response body: {text}");
        serde_json::from_str(&text).map_err(PlankaError::from)
    }

    /// GET a resource, deserializing the response body.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, PlankaError> {
        let url = self.url(path)?;
        debug!("GET {url}");

        let response = self.send("GET", path, self.inner.get(url.clone())).await?;
        self.decode_json("GET", path, response, &url).await
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

        let response = self
            .send("POST", path, self.inner.post(url.clone()).json(body))
            .await?;
        self.decode_json("POST", path, response, &url).await
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

        let response = self
            .send("PATCH", path, self.inner.patch(url.clone()).json(body))
            .await?;
        self.decode_json("PATCH", path, response, &url).await
    }

    /// DELETE a resource. Returns `()` on success.
    pub async fn delete(&self, path: &str) -> Result<(), PlankaError> {
        let url = self.url(path)?;
        debug!("DELETE {url}");

        let response = self
            .send("DELETE", path, self.inner.delete(url.clone()))
            .await?;
        let status = response.status();
        debug!("{status} {url}");

        if !status.is_success() {
            return Err(Self::map_error(
                "DELETE",
                path,
                status,
                &response.text().await.unwrap_or_default(),
            ));
        }

        Ok(())
    }

    /// GET raw bytes (for file downloads).
    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>, PlankaError> {
        let url = self.url(path)?;
        debug!("GET (bytes) {url}");

        let response = self.send("GET", path, self.inner.get(url.clone())).await?;
        let status = response.status();
        debug!("{status} {url}");

        if !status.is_success() {
            return Err(Self::map_error(
                "GET",
                path,
                status,
                &response.text().await.unwrap_or_default(),
            ));
        }

        Ok(response.bytes().await?.to_vec())
    }

    /// POST a multipart form (for file uploads).
    pub async fn post_multipart<T: DeserializeOwned>(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> Result<T, PlankaError> {
        let url = self.url(path)?;
        debug!("POST (multipart) {url}");

        let response = self
            .send("POST", path, self.inner.post(url.clone()).multipart(form))
            .await?;
        self.decode_json("POST", path, response, &url).await
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
