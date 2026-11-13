//! The HTTP client: configuration, authentication, retries and error mapping.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::error::{parse_api_error, Error, Result};
use crate::pagination::{paginate, PageStream};
use crate::resources::{Campaigns, Contacts, Mailboxes, Me, Unibox};

/// Default API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.ayinbox.com/v1";
/// Default per-attempt timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// Default number of automatic retries (three attempts in total).
pub const DEFAULT_MAX_RETRIES: u32 = 2;

const USER_AGENT: &str = concat!("ayinbox-rust/", env!("CARGO_PKG_VERSION"));
const MAX_RETRY_AFTER: u64 = 120;
const BACKOFF_BASE: f64 = 0.5;
const BACKOFF_CAP: f64 = 8.0;

/// Per-call options: idempotency key, timeout override and extra headers.
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    /// Sent as `Idempotency-Key`. Reused on every automatic retry of the call.
    pub idempotency_key: Option<String>,
    /// Overrides the client timeout for each attempt.
    pub timeout: Option<Duration>,
    /// Extra headers, for example `X-Request-Id`.
    pub headers: Vec<(String, String)>,
}

impl RequestOptions {
    /// Empty options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the idempotency key.
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Sets the per-attempt timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Adds a header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// Builder for [`Client`].
#[derive(Default)]
pub struct ClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout: Option<Duration>,
    max_retries: Option<u32>,
    http_client: Option<reqwest::Client>,
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("api_key", &self.api_key.as_ref().map(|_| "[redacted]"))
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}

impl ClientBuilder {
    /// The API key (`aybox_...`) or OAuth access token (`ayat_...`). Defaults to `AYINBOX_API_KEY`.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// The API base URL. Defaults to `AYINBOX_BASE_URL`, then `https://api.ayinbox.com/v1`.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Per-attempt timeout. Defaults to 30 seconds.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Automatic retries. Defaults to 2; 0 disables retries.
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Uses your own `reqwest::Client` (proxies, TLS settings, connection pool).
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Builds the client. Fails when no API key is set and `AYINBOX_API_KEY` is empty.
    pub fn build(self) -> Result<Client> {
        let api_key = self
            .api_key
            .or_else(|| std::env::var("AYINBOX_API_KEY").ok())
            .map(|k| k.trim().to_owned())
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                Error::Config("no API key: pass one or set AYINBOX_API_KEY".to_owned())
            })?;
        let base_url = self
            .base_url
            .or_else(|| std::env::var("AYINBOX_BASE_URL").ok())
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned())
            .trim()
            .trim_end_matches('/')
            .to_owned();
        let mut auth = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| Error::Config("the API key holds invalid characters".to_owned()))?;
        auth.set_sensitive(true);
        let http = match self.http_client {
            Some(c) => c,
            None => reqwest::Client::builder()
                .build()
                .map_err(|e| Error::Config(format!("could not build the HTTP client: {e}")))?,
        };
        Ok(Client {
            inner: Arc::new(Inner {
                http,
                auth,
                base_url,
                timeout: self.timeout.unwrap_or(DEFAULT_TIMEOUT),
                max_retries: self.max_retries.unwrap_or(DEFAULT_MAX_RETRIES),
            }),
            opts: RequestOptions::default(),
        })
    }
}

struct Inner {
    http: reqwest::Client,
    auth: HeaderValue,
    base_url: String,
    timeout: Duration,
    max_retries: u32,
}

/// The AY Inbox API client. Cheap to clone; clones share one connection pool.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
    opts: RequestOptions,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("api_key", &"[redacted]")
            .field("base_url", &self.inner.base_url)
            .field("timeout", &self.inner.timeout)
            .field("max_retries", &self.inner.max_retries)
            .finish()
    }
}

impl Client {
    /// A builder for a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// A client from `AYINBOX_API_KEY` (and `AYINBOX_BASE_URL` when set) with default settings.
    pub fn from_env() -> Result<Self> {
        Self::builder().build()
    }

    /// The base URL in use, without a trailing slash.
    pub fn base_url(&self) -> &str {
        &self.inner.base_url
    }

    /// A view of this client whose calls all carry `opts` (idempotency key, timeout, headers).
    ///
    /// An idempotency key set here is sent on every call made through the returned view,
    /// so make a new view per logical operation.
    pub fn with_options(&self, opts: RequestOptions) -> Client {
        Client {
            inner: Arc::clone(&self.inner),
            opts,
        }
    }

    /// `GET /me`.
    pub fn me(&self) -> Me<'_> {
        Me { client: self }
    }

    /// Campaigns.
    pub fn campaigns(&self) -> Campaigns<'_> {
        Campaigns { client: self }
    }

    /// Contacts.
    pub fn contacts(&self) -> Contacts<'_> {
        Contacts { client: self }
    }

    /// Mailboxes (`/emails`).
    pub fn mailboxes(&self) -> Mailboxes<'_> {
        Mailboxes { client: self }
    }

    /// Unibox (replies from every connected mailbox).
    pub fn unibox(&self) -> Unibox<'_> {
        Unibox { client: self }
    }

    /// Calls any route. `path` is relative to the base URL (for example `/campaigns-overview`).
    ///
    /// Returns the decoded JSON, `Value::Null` for an empty body, or a `Value::String`
    /// holding the raw text of a non-JSON answer.
    pub async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Result<Value> {
        let query: Vec<(String, String)> = query
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let bytes = self.send(method, path, &query, body, false).await?;
        Ok(decode_value(&bytes))
    }

    /// Walks every page of any route answering `{data, pagination}`, yielding one row at a time.
    ///
    /// The same query and body are sent on every page; only `cursor` changes.
    pub fn paginate<T>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> PageStream<T>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        paginate(self.clone(), method, path.to_owned(), query, body)
    }

    pub(crate) async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        body: Option<&Value>,
        read_only: bool,
    ) -> Result<T> {
        let bytes = self.send(method, path, query, body, read_only).await?;
        let slice: &[u8] = if bytes.iter().all(|b| b.is_ascii_whitespace()) {
            b"null"
        } else {
            &bytes
        };
        serde_json::from_slice(slice).map_err(Error::Decode)
    }

    /// Sends with retries. `read_only` marks a POST that only reads (safe to retry).
    pub(crate) async fn send(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        body: Option<&Value>,
        read_only: bool,
    ) -> Result<Vec<u8>> {
        let url = format!("{}/{}", self.inner.base_url, path.trim_start_matches('/'));
        let body_bytes = match body {
            Some(v) => Some(serde_json::to_vec(v).map_err(Error::Decode)?),
            None => None,
        };
        let has_key = self
            .opts
            .idempotency_key
            .as_deref()
            .is_some_and(|k| !k.is_empty());
        let unsafe_method = method == Method::POST || method == Method::PATCH;
        let may_retry_unanswered = !unsafe_method || has_key || read_only;

        let mut attempt: u32 = 0;
        loop {
            let mut req = self
                .inner
                .http
                .request(method.clone(), &url)
                .timeout(self.opts.timeout.unwrap_or(self.inner.timeout))
                .header(reqwest::header::AUTHORIZATION, self.inner.auth.clone())
                .header(reqwest::header::USER_AGENT, USER_AGENT)
                .header(reqwest::header::ACCEPT, "application/json");
            if !query.is_empty() {
                req = req.query(query);
            }
            if has_key {
                req = req.header(
                    "Idempotency-Key",
                    self.opts.idempotency_key.as_deref().unwrap_or_default(),
                );
            }
            let mut extra = HeaderMap::new();
            for (k, v) in &self.opts.headers {
                if let (Ok(name), Ok(value)) = (
                    HeaderName::from_bytes(k.as_bytes()),
                    HeaderValue::from_str(v),
                ) {
                    extra.insert(name, value);
                }
            }
            req = req.headers(extra);
            if let Some(b) = &body_bytes {
                req = req
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(b.clone());
            }

            let can_retry = attempt < self.inner.max_retries;
            match req.send().await {
                Err(e) => {
                    let err = if e.is_timeout() {
                        Error::Timeout(e.without_url())
                    } else {
                        Error::Connection(e.without_url())
                    };
                    if can_retry && may_retry_unanswered {
                        tokio::time::sleep(backoff(attempt)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(err);
                }
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let headers = resp.headers().clone();
                    let bytes = match resp.bytes().await {
                        Ok(b) => b.to_vec(),
                        Err(e) => {
                            let err = if e.is_timeout() {
                                Error::Timeout(e.without_url())
                            } else {
                                Error::Connection(e.without_url())
                            };
                            if can_retry && may_retry_unanswered {
                                tokio::time::sleep(backoff(attempt)).await;
                                attempt += 1;
                                continue;
                            }
                            return Err(err);
                        }
                    };
                    if (200..300).contains(&status) {
                        return Ok(bytes);
                    }
                    let api = parse_api_error(status, headers, &bytes);
                    let retryable = match status {
                        429 => true,
                        502..=504 => may_retry_unanswered,
                        _ => false,
                    } && api.code != "idempotency_outcome_unknown";
                    if can_retry && retryable {
                        let wait = match api.retry_after {
                            Some(s) => Duration::from_secs(s.min(MAX_RETRY_AFTER)),
                            None => backoff(attempt),
                        };
                        tokio::time::sleep(wait).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(Error::from_api(api));
                }
            }
        }
    }
}

fn backoff(attempt: u32) -> Duration {
    let ceiling = (BACKOFF_BASE * 2f64.powi(attempt.min(16) as i32)).min(BACKOFF_CAP);
    let r = RandomState::new().build_hasher().finish();
    let frac = (r >> 11) as f64 / (1u64 << 53) as f64;
    Duration::from_secs_f64(ceiling * frac)
}

fn decode_value(bytes: &[u8]) -> Value {
    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
        return Value::Null;
    }
    serde_json::from_slice(bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(bytes).into_owned()))
}

/// Turns a serializable params struct into query pairs, dropping nulls and joining arrays with commas.
pub(crate) fn to_query<T: Serialize>(params: &T) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(Value::Object(map)) = serde_json::to_value(params) {
        for (k, v) in map {
            if let Some(s) = scalar(&v) {
                out.push((k, s));
            }
        }
    }
    out
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().filter_map(scalar).collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(","))
            }
        }
        Value::Object(_) => None,
    }
}

/// Percent-encodes one path segment.
pub(crate) fn seg(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
