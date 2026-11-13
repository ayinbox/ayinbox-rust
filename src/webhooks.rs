//! Webhook signature verification.
//!
//! Each delivery carries `X-Ayinbox-Signature: t=<unix-seconds>,v1=<hex>`, an HMAC-SHA256 of
//! `<t>.<raw body>` keyed with the endpoint secret (including its `whsec_` prefix).
//! Verify against the raw body bytes, before any JSON parsing.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::types::lenient;

/// Name of the signature header.
pub const SIGNATURE_HEADER: &str = "X-Ayinbox-Signature";
/// Default tolerance between the signed timestamp and now.
pub const DEFAULT_TOLERANCE: Duration = Duration::from_secs(300);

/// Why a signature did not verify.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WebhookError {
    /// The header has no `t` or no `v1`, or `t` is not a number.
    #[error("the signature header is malformed")]
    MalformedHeader,
    /// The timestamp is outside the tolerance.
    #[error("the signature timestamp is outside the tolerance")]
    TimestampOutOfTolerance,
    /// No `v1` signature matched.
    #[error("no signature matched")]
    NoMatch,
    /// The body verified but is not a valid event.
    #[error("the payload is not a valid event: {0}")]
    InvalidPayload(String),
}

/// A webhook delivery body.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookEvent {
    /// Event id; equals `X-Ayinbox-Event-Id`. Use it to drop duplicates.
    #[serde(default, deserialize_with = "lenient")]
    pub id: String,
    /// Event type, for example `campaign.reply_received`.
    #[serde(default, deserialize_with = "lenient")]
    pub event_type: String,
    /// Workspace id.
    #[serde(default, deserialize_with = "lenient")]
    pub organization_id: String,
    /// When the event happened (RFC 3339).
    #[serde(default, deserialize_with = "lenient")]
    pub created_at: String,
    /// Event data.
    #[serde(default)]
    pub data: Value,
    /// Fields this version of the library does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The lowercase hex HMAC-SHA256 of `<timestamp>.<payload>` keyed with `secret`.
pub fn compute_signature(payload: &[u8], timestamp: i64, secret: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes any key");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

/// Verifies a signature header against the raw body, using the current time.
pub fn verify_signature(
    payload: &[u8],
    header: &str,
    secret: &str,
    tolerance: Duration,
) -> Result<(), WebhookError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    verify_signature_at(payload, header, secret, tolerance, now)
}

/// Verifies a signature header against the raw body at a given Unix time.
pub fn verify_signature_at(
    payload: &[u8],
    header: &str,
    secret: &str,
    tolerance: Duration,
    now: i64,
) -> Result<(), WebhookError> {
    let mut timestamp: Option<i64> = None;
    let mut sigs: Vec<&str> = Vec::new();
    for part in header.split(',') {
        let Some((k, v)) = part.trim().split_once('=') else {
            continue;
        };
        match k.trim() {
            "t" => timestamp = v.trim().parse().ok(),
            "v1" => sigs.push(v.trim()),
            _ => {}
        }
    }
    let t = timestamp.ok_or(WebhookError::MalformedHeader)?;
    if sigs.is_empty() {
        return Err(WebhookError::MalformedHeader);
    }
    let tolerance = i64::try_from(tolerance.as_secs()).unwrap_or(i64::MAX);
    if now.saturating_sub(t).saturating_abs() > tolerance {
        return Err(WebhookError::TimestampOutOfTolerance);
    }
    let expected = compute_signature(payload, t, secret);
    let mut matched = subtle::Choice::from(0u8);
    for s in sigs {
        matched |= expected.as_bytes().ct_eq(s.as_bytes());
    }
    if bool::from(matched) {
        Ok(())
    } else {
        Err(WebhookError::NoMatch)
    }
}

/// Verifies the signature, then parses the body as a [`WebhookEvent`].
pub fn construct_event(
    payload: &[u8],
    header: &str,
    secret: &str,
    tolerance: Duration,
) -> Result<WebhookEvent, WebhookError> {
    verify_signature(payload, header, secret, tolerance)?;
    serde_json::from_slice(payload).map_err(|e| WebhookError::InvalidPayload(e.to_string()))
}
