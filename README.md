# AY Inbox Rust library

The official Rust client for the [AY Inbox](https://ayinbox.com) REST API. AY Inbox is a hosted cold email platform: it sends cold email at a paced daily volume, brings every reply into one inbox (Unibox), drafts answers from a per-campaign brain, and can offer a meeting time.

AY Inbox is a hosted service. This repository holds the Rust client library for its public API. The service itself is not open source.

Access to the hosted service is invite only for now: ask at team@ayinbox.com.

## Install

Not on crates.io yet. Until it is, depend on the GitHub repository:

```toml
[dependencies]
ayinbox = { git = "https://github.com/ayinbox/ayinbox-rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
futures-util = "0.3" # for the auto-paging streams
```

Requires Rust 1.75 or later. The client is async, runs on Tokio and uses `reqwest` with rustls.

## Quick start

```rust
use ayinbox::{CampaignListParams, Client, ContactInput, Error, RequestOptions};
use futures_util::TryStreamExt;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let client = Client::from_env()?; // reads AYINBOX_API_KEY
    println!("Signed in as {}", client.me().get().await?.email);

    let mut campaigns = client.campaigns().list_all(&CampaignListParams::default());
    while let Some(c) = campaigns.try_next().await? {
        println!("{}: {}", c.name, c.status);
    }

    let lee = ContactInput { email: "lee@example.com".into(), first_name: Some("Lee".into()), ..Default::default() };
    match client.with_options(RequestOptions::new().idempotency_key("import-lee-2026-11-13"))
        .contacts().create(&[lee]).await
    {
        Ok(saved) => println!("Saved {}", saved[0].id),
        Err(e @ Error::PermissionDenied(_)) => eprintln!("Missing scope: {:?} (request {:?})", e.code(), e.request_id()),
        Err(e) => return Err(e),
    }
    Ok(())
}
```

Response types keep fields the library does not model in `extra`, and a missing or `null` field becomes its default, so a new server field never breaks decoding. Fields the API reference has not pinned down are `serde_json::Value`. Timestamps are RFC 3339 strings.

## Client options

```rust
use std::time::Duration;

let client = ayinbox::Client::builder()
    .api_key("aybox_...")                       // default: AYINBOX_API_KEY
    .base_url("https://api.ayinbox.com/v1")     // default, or AYINBOX_BASE_URL
    .timeout(Duration::from_secs(30))           // per attempt
    .max_retries(2)                             // 0 disables retries
    .http_client(reqwest::Client::new())        // optional, your own reqwest client
    .build()?;
```

API keys start with `aybox_` and OAuth access tokens with `ayat_`; both work as `api_key`. The key is sent as `Authorization: Bearer <key>`, marked sensitive, and never appears in `Debug` output or error messages.

Per-call options (idempotency key, timeout, extra headers such as `X-Request-Id`) go on a view of the client:

```rust
let view = client.with_options(
    RequestOptions::new()
        .idempotency_key("pause-q4-1")
        .timeout(Duration::from_secs(10))
        .header("X-Request-Id", "my-trace-id"),
);
view.campaigns().pause(campaign_id).await?;
```

## Resources

| Resource | Method | HTTP |
|---|---|---|
| `me()` | `get()` | `GET /me` |
| `campaigns()` | `list(&params)`, `list_all(&params)` | `GET /campaigns` |
| | `get(id)` | `GET /campaigns/{id}` |
| | `create(&params)` | `POST /campaigns` |
| | `pause(id)` | `POST /campaigns/{id}/stop` |
| `contacts()` | `list(&params)`, `list_all(&params)` | `POST /contacts/search` |
| | `get(id)` | `GET /contacts/{id}` |
| | `create(&[contact])` | `POST /contacts` |
| | `update(id, &params)` | `PATCH /contacts/{id}` |
| | `delete(id)` | `DELETE /contacts/{id}` |
| `mailboxes()` | `list(&params)`, `list_all(&params)` | `GET /emails` |
| `unibox()` | `list_threads(&params)`, `list_threads_all(&params)` | `GET /unibox` |
| | `get_thread(thread_id, &params)` | `GET /unibox/thread` |
| `ayinbox::webhooks` | `verify_signature`, `construct_event` | local, no HTTP |

For `contacts().list`, `limit`, `cursor` and `category` go in the query string and every other field is the search body. `contacts().create` always returns a `Vec`, even for one contact; an address that already exists is updated, not duplicated. Params structs that take arbitrary fields have an `extra` map for anything not modelled.

Every other route is reachable through `request`:

```rust
use ayinbox::Method;
use serde_json::json;

let overview = client.request(Method::GET, "/campaigns-overview", &[], None).await?;
client
    .with_options(RequestOptions::new().idempotency_key("suppress-competitor"))
    .request(Method::POST, "/suppressions", &[], Some(&json!({"entries": [{"value": "competitor.example"}]})))
    .await?;
```

`request(method, path, query, body)` goes through the same authentication, retries and error mapping, and returns a `serde_json::Value`: the decoded JSON, `Value::Null` for an empty body, or a `Value::String` with the raw text of a non-JSON answer.

## Pagination

`list` methods return a `Page<T>` with `data`, `pagination` (`next_cursor`, `has_more`, `total` when the endpoint sends it) and `extra` for other top-level fields (for example `counts` on a contact search). `page.next_cursor()` gives the cursor to pass back.

`list_all` methods and `client.paginate(method, path, query, body)` return a `Stream` of rows that fetches pages as you read, passing the cursor back unchanged. A 429 mid-walk is waited out by the retry logic. For a search, the same body is sent on every page, as the API requires. Use `limit` between 1 and 100.

```rust
let rows: Vec<serde_json::Value> = client
    .paginate(Method::GET, "/suppressions", &[("limit", "200")], None)
    .try_collect()
    .await?;
```

## Errors

Every call returns `Result<T, ayinbox::Error>`. An answer with a non-2xx status becomes a variant holding an `ApiError` with `status`, `code`, `message`, `reason`, `request_id`, `retry_after`, `body` and `headers`. Branch on the variant and `code`, never on `message`.

| Variant | Status |
|---|---|
| `BadRequest` | 400 |
| `Authentication` | 401 |
| `PaymentRequired` | 402 |
| `PermissionDenied` | 403 |
| `NotFound` | 404 |
| `Conflict` | 409 |
| `Unprocessable` | 422 |
| `RateLimit` (has `retry_after` seconds) | 429 |
| `InternalServer` | 500 and other 5xx |
| `ServiceUnavailable` (may have `retry_after`) | 503 |
| `Api` | any other status |
| `Connection`, `Timeout` | no answer |
| `Decode`, `Config` | local |

Helpers: `status()`, `code()`, `request_id()`, `retry_after()`, `api_error()`, `is_not_found()`, `is_rate_limited()`, `is_validation()` (400 or 422), `is_authentication()`, `is_permission_denied()`, `is_conflict()`, `is_server_error()`, `is_network()`.

A rate-limit answer without a `code` (the per-category limiter) is read as `rate_limit_exceeded`, and its `retry_after_ms` becomes `retry_after` in seconds.

## Retries and timeouts

The client retries `429`, `502`, `503`, `504`, connection failures and timeouts, up to `max_retries` times (default 2, so three attempts). It waits for `Retry-After` when the server gives one (capped at 120 seconds), otherwise exponential backoff with full jitter (base 0.5 seconds, cap 8 seconds).

A `POST` or `PATCH` without an idempotency key is retried only on `429`, which the server never ran. A `500` is never retried, and neither is `409 idempotency_outcome_unknown`. `contacts().list` and the auto-pagers are reads even when they use `POST`, so they are retried like a `GET`.

The timeout applies to each attempt; set it per call with `RequestOptions::timeout`.

## Idempotency

Every call accepts an idempotency key through `with_options(RequestOptions::new().idempotency_key(...))`, sent as the `Idempotency-Key` header. The library does not generate keys for you: pass one on any write you might retry, and the client reuses it on every automatic retry of that call. A view made with `with_options` sends its key on every call made through it, so make a new view per operation.

The server keeps the result for 24 hours and replays it for the same key and the same request. The same key with a different request answers `409 conflict`; a first attempt that never finished answers `409 idempotency_outcome_unknown`, and you should check whether the change happened before retrying with a new key.

## Webhooks

Each delivery carries `X-Ayinbox-Signature: t=<unix-seconds>,v1=<hex>`. Verify it against the raw request body, before any JSON parsing:

```rust
use ayinbox::webhooks::{construct_event, verify_signature, DEFAULT_TOLERANCE};

let secret = std::env::var("AYINBOX_WEBHOOK_SECRET")?; // whsec_..., shown once when the endpoint is created
let event = construct_event(&raw_body, signature_header, &secret, DEFAULT_TOLERANCE)?;
// or: verify_signature(&raw_body, signature_header, &secret, DEFAULT_TOLERANCE)?;
println!("{} {}", event.event_type, event.id);
```

The default tolerance is 300 seconds. The comparison is constant time. Use `X-Ayinbox-Event-Id` (also `event.id`) to drop duplicate deliveries.

## Calls that send email

- These calls put real email on the wire:
  - `campaigns.start` (`POST /campaigns/{id}/start`): needs `START_CAMPAIGNS` or `SEND_CAMPAIGNS`.
  - `campaigns.update` with `status: "active"` (and `completed` or `paused_undeliverable`): needs `START_CAMPAIGNS` or `SEND_CAMPAIGNS` in addition to `WRITE_CAMPAIGNS`.
  - `campaigns.leads.resume` on a finished campaign: needs `START_CAMPAIGNS` or `SEND_CAMPAIGNS`.
  - `unibox.reply` and `unibox.approveAgentDraft`: need `WRITE_UNIBOX`. A key with `WRITE_UNIBOX` can send replies from any mailbox it can reach.
  - Through `request`: `POST /unibox/compose`, `POST /emails/{id}/send`, `POST /campaigns/{id}/test-email`, the first-email review queue (`/campaigns/{id}/reviews/...`), placement tests: `SEND_CAMPAIGNS` (compose: `WRITE_UNIBOX`).

  In this library, all of those are reached through `request`. The named methods here never send: `campaigns().create` makes a draft and `campaigns().pause` stops sending.
- Adding contacts can send mail without calling start: contacts added to an `active` campaign are queued; a `completed` campaign starts again on its own when leads are added; a contact that comes to match a segment linked to a sending campaign is enrolled. That includes `contacts().create` and `contacts().update` with `campaigns`.
- `START_CAMPAIGNS` starts, resumes and pauses campaigns and nothing else. `SEND_CAMPAIGNS` covers everything `START_CAMPAIGNS` covers plus the other sending actions. `WRITE_CAMPAIGNS` alone edits drafts and pauses, and cannot start anything.
- A key for a model that reads the inbox should use the "Agent: read and draft" preset (84574239): every read scope plus `AI_DRAFT`, with nothing that writes or sends.
- Some calls spend workspace credits (AI drafts, research, enrichment, paid tools). They answer `402 insufficient_credits` when the balance is out.
- Retrying a send without an idempotency key can send twice. Pass an idempotency key on every mutating call you might retry.

## Examples

- [`examples/list_campaigns.rs`](examples/list_campaigns.rs): sign in and list every campaign (`cargo run --example list_campaigns`).
- [`examples/verify_webhook.rs`](examples/verify_webhook.rs): verify a signed delivery (`cargo run --example verify_webhook`).

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The tests run against a local `wiremock` server fed from `tests/fixtures/`. They make no network calls and use no real key.

## Documentation

- API reference: https://docs.ayinbox.com/api/
- Authentication and scopes: https://docs.ayinbox.com/api/authentication/
- Errors: https://docs.ayinbox.com/api/error-codes/

## License

MIT. See [LICENSE](LICENSE).
