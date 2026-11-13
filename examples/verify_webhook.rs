//! Verifies a signed delivery.
//! Run: AYINBOX_WEBHOOK_SECRET=whsec_... cargo run --example verify_webhook

use ayinbox::webhooks::{compute_signature, construct_event, DEFAULT_TOLERANCE};

fn main() {
    let secret =
        std::env::var("AYINBOX_WEBHOOK_SECRET").unwrap_or_else(|_| "whsec_test_secret".into());

    // In a real handler these come from the request: the raw body bytes and the
    // X-Ayinbox-Signature header. Here a delivery is signed locally to show the flow.
    let raw_body = br#"{"id":"00000000-0000-4000-8000-000000000001","event_type":"campaign.reply_received","organization_id":"00000000-0000-4000-8000-0000000000aa","created_at":"2023-11-14T22:13:20Z","data":{}}"#;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after 1970")
        .as_secs() as i64;
    let header = format!("t={now},v1={}", compute_signature(raw_body, now, &secret));

    match construct_event(raw_body, &header, &secret, DEFAULT_TOLERANCE) {
        Ok(event) => println!("Verified {} (event id {})", event.event_type, event.id),
        Err(e) => eprintln!("Signature did not verify ({e}); answer 400 and drop the delivery"),
    }
}
