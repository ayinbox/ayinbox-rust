//! Error types returned by the client.

use reqwest::header::HeaderMap;
use serde_json::Value;

/// The details of an error answer from the API (any non-2xx status).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ApiError {
    /// HTTP status code.
    pub status: u16,
    /// Stable machine key, for example `not_found` or `api_key_holder_left`. Branch on this.
    pub code: String,
    /// Human text. Never branch on it.
    pub message: String,
    /// Finer key some codes carry.
    pub reason: Option<String>,
    /// Request id from the body or the `X-Request-Id` header.
    pub request_id: Option<String>,
    /// Seconds to wait before retrying, from `Retry-After`, `retry_after` or `retry_after_ms`.
    pub retry_after: Option<u64>,
    /// The decoded body, or a JSON string holding the raw text when the body was not JSON.
    pub body: Value,
    /// Response headers.
    pub headers: HeaderMap,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.status, self.code, self.message)?;
        if let Some(id) = &self.request_id {
            write!(f, " (request id {id})")?;
        }
        Ok(())
    }
}

/// Every error the client returns.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// 400: the request was malformed or a field was invalid.
    #[error("bad request: {0}")]
    BadRequest(Box<ApiError>),
    /// 401: the credential is missing, wrong or no longer valid.
    #[error("authentication failed: {0}")]
    Authentication(Box<ApiError>),
    /// 402: the workspace is out of credits or not entitled.
    #[error("payment required: {0}")]
    PaymentRequired(Box<ApiError>),
    /// 403: the credential lacks a scope or the workspace refuses the action.
    #[error("permission denied: {0}")]
    PermissionDenied(Box<ApiError>),
    /// 404: the resource does not exist or is not visible to this credential.
    #[error("not found: {0}")]
    NotFound(Box<ApiError>),
    /// 409: a conflict, including idempotency key conflicts.
    #[error("conflict: {0}")]
    Conflict(Box<ApiError>),
    /// 422: the request was well formed but could not be processed.
    #[error("unprocessable: {0}")]
    Unprocessable(Box<ApiError>),
    /// 429: rate limited. `retry_after` says how long to wait.
    #[error("rate limited: {0}")]
    RateLimit(Box<ApiError>),
    /// 500, 501, 502, 504 and any other 5xx except 503.
    #[error("server error: {0}")]
    InternalServer(Box<ApiError>),
    /// 503: the service or a dependency is unavailable.
    #[error("service unavailable: {0}")]
    ServiceUnavailable(Box<ApiError>),
    /// Any other non-2xx status.
    #[error("api error: {0}")]
    Api(Box<ApiError>),
    /// The request could not reach the server (DNS, TLS, connection).
    #[error("connection error: {0}")]
    Connection(#[source] reqwest::Error),
    /// The request timed out.
    #[error("request timed out: {0}")]
    Timeout(#[source] reqwest::Error),
    /// A 2xx body could not be decoded into the expected type.
    #[error("could not decode the response: {0}")]
    Decode(#[source] serde_json::Error),
    /// The client was configured wrongly (for example, no API key).
    #[error("configuration error: {0}")]
    Config(String),
}

impl Error {
    pub(crate) fn from_api(e: ApiError) -> Self {
        let b = Box::new(e);
        match b.status {
            400 => Error::BadRequest(b),
            401 => Error::Authentication(b),
            402 => Error::PaymentRequired(b),
            403 => Error::PermissionDenied(b),
            404 => Error::NotFound(b),
            409 => Error::Conflict(b),
            422 => Error::Unprocessable(b),
            429 => Error::RateLimit(b),
            503 => Error::ServiceUnavailable(b),
            500..=599 => Error::InternalServer(b),
            _ => Error::Api(b),
        }
    }

    /// The API error details, when the server answered.
    pub fn api_error(&self) -> Option<&ApiError> {
        match self {
            Error::BadRequest(e)
            | Error::Authentication(e)
            | Error::PaymentRequired(e)
            | Error::PermissionDenied(e)
            | Error::NotFound(e)
            | Error::Conflict(e)
            | Error::Unprocessable(e)
            | Error::RateLimit(e)
            | Error::InternalServer(e)
            | Error::ServiceUnavailable(e)
            | Error::Api(e) => Some(e),
            _ => None,
        }
    }

    /// HTTP status, when the server answered.
    pub fn status(&self) -> Option<u16> {
        self.api_error().map(|e| e.status)
    }

    /// The API's machine-readable `code`, when the server answered.
    pub fn code(&self) -> Option<&str> {
        self.api_error().map(|e| e.code.as_str())
    }

    /// The request id, when known.
    pub fn request_id(&self) -> Option<&str> {
        self.api_error().and_then(|e| e.request_id.as_deref())
    }

    /// Seconds the server asked to wait before retrying, when given.
    pub fn retry_after(&self) -> Option<u64> {
        self.api_error().and_then(|e| e.retry_after)
    }

    /// True for a 404.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound(_))
    }

    /// True for a 429.
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, Error::RateLimit(_))
    }

    /// True for a 400 or 422.
    pub fn is_validation(&self) -> bool {
        matches!(self, Error::BadRequest(_) | Error::Unprocessable(_))
    }

    /// True for a 401.
    pub fn is_authentication(&self) -> bool {
        matches!(self, Error::Authentication(_))
    }

    /// True for a 403.
    pub fn is_permission_denied(&self) -> bool {
        matches!(self, Error::PermissionDenied(_))
    }

    /// True for a 409.
    pub fn is_conflict(&self) -> bool {
        matches!(self, Error::Conflict(_))
    }

    /// True for any 5xx.
    pub fn is_server_error(&self) -> bool {
        matches!(
            self,
            Error::InternalServer(_) | Error::ServiceUnavailable(_)
        )
    }

    /// True when the request never got an answer (connection failure or timeout).
    pub fn is_network(&self) -> bool {
        matches!(self, Error::Connection(_) | Error::Timeout(_))
    }
}

/// Result alias for this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn parse_api_error(status: u16, headers: HeaderMap, bytes: &[u8]) -> ApiError {
    let text = String::from_utf8_lossy(bytes).into_owned();
    let parsed: Option<Value> = serde_json::from_slice(bytes).ok();
    let obj = parsed.as_ref().and_then(|v| v.as_object());
    let get_str = |k: &str| {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
    };

    let code = get_str("code")
        .filter(|c| !c.is_empty())
        .or_else(|| {
            get_str("error").filter(|e| {
                !e.is_empty()
                    && e.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            })
        })
        .unwrap_or_else(|| default_code(status).to_owned());

    let request_id = get_str("request_id").or_else(|| {
        headers
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    });

    let retry_after = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .or_else(|| {
            obj.and_then(|o| o.get("retry_after"))
                .and_then(|v| v.as_u64())
        })
        .or_else(|| {
            obj.and_then(|o| o.get("retry_after_ms"))
                .and_then(|v| v.as_u64())
                .map(|ms| ms.div_ceil(1000))
        });

    let message = get_str("message").unwrap_or_else(|| text.clone());
    let reason = get_str("reason");
    let body = parsed.unwrap_or(Value::String(text));

    ApiError {
        status,
        code,
        message,
        reason,
        request_id,
        retry_after,
        body,
        headers,
    }
}

fn default_code(status: u16) -> &'static str {
    match status {
        400 => "bad_request",
        401 => "unauthorized",
        402 => "insufficient_credits",
        403 => "forbidden",
        404 => "not_found",
        409 => "conflict",
        422 => "unprocessable",
        429 => "rate_limit_exceeded",
        500 => "internal_error",
        501 => "not_implemented",
        503 => "service_unavailable",
        _ => "unknown_error",
    }
}
