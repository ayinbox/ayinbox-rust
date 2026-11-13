use std::time::Duration;

use ayinbox::webhooks::{
    compute_signature, construct_event, verify_signature_at, WebhookError, DEFAULT_TOLERANCE,
};
use serde_json::Value;

struct Vector {
    secret: String,
    timestamp: i64,
    body: String,
    expected: String,
    header: String,
}

fn vector() -> Vector {
    let p = format!(
        "{}/tests/fixtures/webhook_signature.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    Vector {
        secret: v["secret"].as_str().unwrap().into(),
        timestamp: v["timestamp"].as_i64().unwrap(),
        body: v["body"].as_str().unwrap().into(),
        expected: v["expected_v1"].as_str().unwrap().into(),
        header: v["header"].as_str().unwrap().into(),
    }
}

#[test]
fn computes_the_test_vector() {
    let v = vector();
    assert_eq!(
        compute_signature(v.body.as_bytes(), v.timestamp, &v.secret),
        v.expected
    );
}

#[test]
fn valid_signature_passes() {
    let v = vector();
    verify_signature_at(
        v.body.as_bytes(),
        &v.header,
        &v.secret,
        DEFAULT_TOLERANCE,
        v.timestamp,
    )
    .unwrap();
}

#[test]
fn tampered_body_fails() {
    let v = vector();
    let body = v.body.replace("reply_received", "reply_receivee");
    assert_eq!(
        verify_signature_at(
            body.as_bytes(),
            &v.header,
            &v.secret,
            DEFAULT_TOLERANCE,
            v.timestamp
        ),
        Err(WebhookError::NoMatch)
    );
}

#[test]
fn wrong_secret_fails() {
    let v = vector();
    assert_eq!(
        verify_signature_at(
            v.body.as_bytes(),
            &v.header,
            "whsec_other",
            DEFAULT_TOLERANCE,
            v.timestamp
        ),
        Err(WebhookError::NoMatch)
    );
}

#[test]
fn stale_timestamp_fails() {
    let v = vector();
    assert_eq!(
        verify_signature_at(
            v.body.as_bytes(),
            &v.header,
            &v.secret,
            DEFAULT_TOLERANCE,
            v.timestamp + 301
        ),
        Err(WebhookError::TimestampOutOfTolerance)
    );
}

#[test]
fn malformed_headers_fail() {
    let v = vector();
    for h in ["", "v1=abc", "t=1700000000", "t=abc,v1=abc"] {
        assert_eq!(
            verify_signature_at(
                v.body.as_bytes(),
                h,
                &v.secret,
                DEFAULT_TOLERANCE,
                v.timestamp
            ),
            Err(WebhookError::MalformedHeader),
            "{h}"
        );
    }
}

#[test]
fn any_matching_v1_passes() {
    let v = vector();
    let header = format!("t={},v0=zz,v1=deadbeef,v1={}", v.timestamp, v.expected);
    verify_signature_at(
        v.body.as_bytes(),
        &header,
        &v.secret,
        Duration::from_secs(300),
        v.timestamp,
    )
    .unwrap();
}

#[test]
fn construct_event_parses_a_fresh_delivery() {
    let v = vector();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let header = format!(
        "t={now},v1={}",
        compute_signature(v.body.as_bytes(), now, &v.secret)
    );
    let event = construct_event(v.body.as_bytes(), &header, &v.secret, DEFAULT_TOLERANCE).unwrap();
    assert_eq!(event.event_type, "campaign.reply_received");
    assert_eq!(event.id, "00000000-0000-4000-8000-000000000001");
}
