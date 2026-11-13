use std::time::Duration;

use ayinbox::{
    CampaignCreate, CampaignListParams, Client, ContactInput, ContactListParams, ContactUpdate,
    Error, MailboxListParams, Method, RequestOptions, ThreadGetParams, ThreadListParams,
};
use futures_util::TryStreamExt;
use serde_json::{json, Value};
use wiremock::matchers::{body_json, header, header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const KEY: &str = "aybox_test_key_not_real";

fn fixture(name: &str) -> Value {
    let p = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn ok(name: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(fixture(name))
        .insert_header("X-Request-Id", "req-fixture-0001")
}

fn err(status: u16, name: &str) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(fixture(&format!("errors/{name}")))
}

fn client(server: &MockServer) -> Client {
    Client::builder()
        .api_key(KEY)
        .base_url(format!("{}/v1/", server.uri()))
        .build()
        .unwrap()
}

fn no_retry(server: &MockServer) -> Client {
    Client::builder()
        .api_key(KEY)
        .base_url(format!("{}/v1", server.uri()))
        .max_retries(0)
        .build()
        .unwrap()
}

#[tokio::test]
async fn sends_auth_user_agent_and_accept() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .and(header("authorization", format!("Bearer {KEY}").as_str()))
        .and(header(
            "user-agent",
            concat!("ayinbox-rust/", env!("CARGO_PKG_VERSION")),
        ))
        .and(header("accept", "application/json"))
        .respond_with(ok("me.json"))
        .expect(1)
        .mount(&server)
        .await;

    let me = client(&server).me().get().await.unwrap();
    assert_eq!(me.email, "dana@example.com");
    assert_eq!(me.auth_type, "api_key");
    assert_eq!(me.scopes.len(), 3);
    assert_eq!(me.organization_name.as_deref(), Some("Example Co"));
}

#[tokio::test]
async fn debug_output_redacts_the_key() {
    let c = Client::builder()
        .api_key(KEY)
        .base_url("http://x")
        .build()
        .unwrap();
    let s = format!("{c:?}");
    assert!(!s.contains(KEY));
    assert!(s.contains("redacted"));
}

#[tokio::test]
async fn missing_key_is_a_config_error() {
    std::env::remove_var("AYINBOX_API_KEY");
    let e = Client::builder().build().unwrap_err();
    assert!(matches!(e, Error::Config(_)));
}

#[tokio::test]
async fn maps_each_error_fixture() {
    type Case = (u16, &'static str, &'static str, fn(&Error) -> bool);
    let cases: &[Case] = &[
        (400, "400_bad_request.json", "bad_request", |e| {
            matches!(e, Error::BadRequest(_)) && e.is_validation()
        }),
        (400, "400_invalid_cursor.json", "bad_request", |e| {
            matches!(e, Error::BadRequest(_))
        }),
        (401, "401_unauthorized.json", "unauthorized", |e| {
            e.is_authentication()
        }),
        (
            401,
            "401_api_key_holder_left.json",
            "api_key_holder_left",
            |e| matches!(e, Error::Authentication(_)),
        ),
        (
            402,
            "402_insufficient_credits.json",
            "insufficient_credits",
            |e| matches!(e, Error::PaymentRequired(_)),
        ),
        (403, "403_forbidden.json", "forbidden", |e| {
            e.is_permission_denied()
        }),
        (
            403,
            "403_api_key_mailbox_limited.json",
            "api_key_mailbox_limited",
            |e| matches!(e, Error::PermissionDenied(_)),
        ),
        (404, "404_not_found.json", "not_found", |e| e.is_not_found()),
        (409, "409_conflict.json", "conflict", |e| e.is_conflict()),
        (
            409,
            "409_idempotency_outcome_unknown.json",
            "idempotency_outcome_unknown",
            |e| matches!(e, Error::Conflict(_)),
        ),
        (
            409,
            "409_idempotent_secret_withheld.json",
            "idempotent_secret_withheld",
            |e| matches!(e, Error::Conflict(_)),
        ),
        (422, "422_unprocessable.json", "unprocessable", |e| {
            matches!(e, Error::Unprocessable(_)) && e.is_validation()
        }),
        (500, "500_internal_error.json", "internal_error", |e| {
            matches!(e, Error::InternalServer(_)) && e.is_server_error()
        }),
        (
            503,
            "503_service_unavailable.json",
            "service_unavailable",
            |e| matches!(e, Error::ServiceUnavailable(_)),
        ),
    ];
    for (status, file, code, check) in cases {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/campaigns/abc"))
            .respond_with(err(*status, file))
            .mount(&server)
            .await;
        let e = no_retry(&server).campaigns().get("abc").await.unwrap_err();
        assert!(check(&e), "{file}: {e:?}");
        assert_eq!(e.status(), Some(*status), "{file}");
        assert_eq!(e.code(), Some(*code), "{file}");
        assert_eq!(e.request_id(), Some("req-fixture-0001"), "{file}");
        assert!(!e.to_string().contains(KEY));
    }
}

#[tokio::test]
async fn rate_limit_with_code_reads_retry_after_from_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(err(429, "429_api_key_rate_limited.json"))
        .mount(&server)
        .await;
    let e = no_retry(&server).me().get().await.unwrap_err();
    assert!(e.is_rate_limited());
    assert_eq!(e.code(), Some("rate_limit_exceeded"));
    assert_eq!(e.retry_after(), Some(12));
}

#[tokio::test]
async fn rate_limit_without_code_uses_error_and_retry_after_ms() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            err(429, "429_category_rate_limited.json").insert_header("X-Request-Id", "hdr-id"),
        )
        .mount(&server)
        .await;
    let e = no_retry(&server).me().get().await.unwrap_err();
    assert!(matches!(e, Error::RateLimit(_)));
    assert_eq!(e.code(), Some("rate_limit_exceeded"));
    assert_eq!(e.retry_after(), Some(12));
    assert_eq!(e.request_id(), Some("hdr-id"));
}

#[tokio::test]
async fn non_json_error_body_keeps_text_and_default_code() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(502).set_body_string("bad gateway"))
        .mount(&server)
        .await;
    let e = no_retry(&server).me().get().await.unwrap_err();
    assert!(matches!(e, Error::InternalServer(_)));
    assert_eq!(e.code(), Some("unknown_error"));
    assert_eq!(e.api_error().unwrap().message, "bad gateway");
}

#[tokio::test]
async fn retries_503_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(err(503, "503_service_unavailable.json"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(ok("me.json"))
        .expect(1)
        .mount(&server)
        .await;
    let me = client(&server).me().get().await.unwrap();
    assert_eq!(me.first_name, "Dana");
}

#[tokio::test]
async fn retries_429_honouring_retry_after_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/campaigns"))
        .respond_with(err(429, "429_category_rate_limited.json").insert_header("Retry-After", "1"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/campaigns"))
        .respond_with(ok("campaign.json"))
        .expect(1)
        .mount(&server)
        .await;
    let started = std::time::Instant::now();
    // No idempotency key: a 429 never ran, so it is still retried.
    let c = client(&server)
        .campaigns()
        .create(&CampaignCreate {
            name: "Q4 outreach".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(started.elapsed() >= Duration::from_millis(900));
    assert_eq!(c.name, "Q4 outreach");
}

#[tokio::test]
async fn does_not_retry_a_keyless_post_on_503() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/campaigns"))
        .respond_with(err(503, "503_service_unavailable.json"))
        .expect(1)
        .mount(&server)
        .await;
    let e = client(&server)
        .campaigns()
        .create(&CampaignCreate {
            name: "x".into(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(matches!(e, Error::ServiceUnavailable(_)));
}

#[tokio::test]
async fn retries_a_post_with_an_idempotency_key_and_reuses_it() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/contacts"))
        .and(header("idempotency-key", "import-lee-1"))
        .respond_with(err(503, "503_service_unavailable.json"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/contacts"))
        .and(header("idempotency-key", "import-lee-1"))
        .and(header("content-type", "application/json"))
        .and(body_json(
            json!([{"email": "lee@example.com", "first_name": "Lee"}]),
        ))
        .respond_with(ok("contacts_create.json"))
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server);
    let saved = c
        .with_options(RequestOptions::new().idempotency_key("import-lee-1"))
        .contacts()
        .create(&[ContactInput {
            email: "lee@example.com".into(),
            first_name: Some("Lee".into()),
            ..Default::default()
        }])
        .await
        .unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].email, "lee@example.com");
}

#[tokio::test]
async fn never_retries_500() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(err(500, "500_internal_error.json"))
        .expect(1)
        .mount(&server)
        .await;
    assert!(client(&server).me().get().await.is_err());
}

#[tokio::test]
async fn connection_error_is_its_own_variant() {
    let c = Client::builder()
        .api_key(KEY)
        .base_url("http://127.0.0.1:1/v1")
        .max_retries(0)
        .build()
        .unwrap();
    let e = c.me().get().await.unwrap_err();
    assert!(e.is_network(), "{e:?}");
    assert!(!format!("{e:?}").contains(KEY));
}

#[tokio::test]
async fn auto_pages_across_two_pages() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/campaigns"))
        .and(query_param("cursor", "c1_ZmFrZS1jdXJzb3ItcGFnZS0y"))
        .and(query_param("status", "active"))
        .respond_with(ok("campaigns_list_page2.json"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/campaigns"))
        .and(query_param("status", "active"))
        .respond_with(ok("campaigns_list.json"))
        .expect(1)
        .mount(&server)
        .await;
    let all: Vec<_> = client(&server)
        .campaigns()
        .list_all(&CampaignListParams {
            status: Some("active".into()),
            ..Default::default()
        })
        .try_collect()
        .await
        .unwrap();
    let first = fixture("campaigns_list.json")["data"]
        .as_array()
        .unwrap()
        .len();
    let second = fixture("campaigns_list_page2.json")["data"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(all.len(), first + second);
    assert_eq!(all.last().unwrap().name, "Partner intro");
}

#[tokio::test]
async fn one_page_exposes_cursor_and_total() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/campaigns"))
        .and(query_param("limit", "2"))
        .respond_with(ok("campaigns_list.json"))
        .mount(&server)
        .await;
    let page = client(&server)
        .campaigns()
        .list(&CampaignListParams {
            limit: Some(2),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page.next_cursor(), Some("c1_ZmFrZS1jdXJzb3ItcGFnZS0y"));
    assert_eq!(page.pagination.total, Some(3));
}

#[tokio::test]
async fn campaigns_get_and_pause() {
    let server = MockServer::start().await;
    let id = "11111111-1111-4111-8111-111111111111";
    Mock::given(method("GET"))
        .and(path(format!("/v1/campaigns/{id}")))
        .respond_with(ok("campaign.json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/campaigns/{id}/stop")))
        .and(header("idempotency-key", "pause-1"))
        .respond_with(ok("campaign_stop.json"))
        .mount(&server)
        .await;
    let c = client(&server);
    let campaign = c.campaigns().get(id).await.unwrap();
    assert_eq!(campaign.id, id);
    assert_eq!(campaign.daily_limit, 50);
    assert!(campaign.start_date.is_none());
    let r = c
        .with_options(RequestOptions::new().idempotency_key("pause-1"))
        .campaigns()
        .pause(id)
        .await
        .unwrap();
    assert_eq!(r.status, "stopped");
}

#[tokio::test]
async fn contacts_list_sends_body_and_query() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/contacts/search"))
        .and(query_param("limit", "10"))
        .and(body_json(json!({"query": "lee", "sort_by": "email"})))
        .respond_with(ok("contacts_list.json"))
        .expect(1)
        .mount(&server)
        .await;
    let page = client(&server)
        .contacts()
        .list(&ContactListParams {
            limit: Some(10),
            query: Some("lee".into()),
            sort_by: Some("email".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!page.data.is_empty());
    assert_eq!(page.data[0].custom_fields["title"], "Head of Ops");
    assert!(page.next_cursor().is_none());
}

#[tokio::test]
async fn contacts_get_update_delete() {
    let server = MockServer::start().await;
    let id = "22222222-2222-4222-8222-111111111111";
    Mock::given(method("GET"))
        .and(path(format!("/v1/contacts/{id}")))
        .respond_with(ok("contact.json"))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/contacts/{id}")))
        .and(body_json(json!({"company": "New Co"})))
        .respond_with(ok("contact.json"))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("/v1/contacts/{id}")))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let c = client(&server);
    let contact = c.contacts().get(id).await.unwrap();
    assert_eq!(contact.categories[0].title, "VIP");
    let updated = c
        .contacts()
        .update(
            id,
            &ContactUpdate {
                company: Some("New Co".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.id, id);
    c.contacts().delete(id).await.unwrap();
}

#[tokio::test]
async fn mailboxes_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/emails"))
        .respond_with(ok("mailboxes_list.json"))
        .mount(&server)
        .await;
    let page = client(&server)
        .mailboxes()
        .list(&MailboxListParams::default())
        .await
        .unwrap();
    assert_eq!(page.data[0].email, "sales@example.com");
    assert_eq!(page.data[0].provider, "gmail");
    // Unmodelled fields are kept.
    assert!(page.data[0].extra.contains_key("signature_sync"));
}

#[tokio::test]
async fn unibox_list_and_thread() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/unibox"))
        .and(query_param("unseen", "true"))
        .respond_with(ok("unibox_threads.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/unibox/thread"))
        .and(query_param("thread_id", "thread-example-001"))
        .respond_with(ok("unibox_thread.json"))
        .mount(&server)
        .await;
    let c = client(&server);
    let threads = c
        .unibox()
        .list_threads(&ThreadListParams {
            unseen: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(threads.data[0].thread_id, "thread-example-001");
    assert!(threads.pagination.has_more);
    let thread = c
        .unibox()
        .get_thread("thread-example-001", &ThreadGetParams::default())
        .await
        .unwrap();
    assert!(thread.data.len() >= 2);
}

#[tokio::test]
async fn generic_request_and_paginate() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/campaigns-overview"))
        .and(header("x-request-id", "trace-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"total": 3})))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/suppressions/abc"))
        .and(header_exists("idempotency-key"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/emails"))
        .respond_with(ok("mailboxes_list.json"))
        .mount(&server)
        .await;
    let c = client(&server);
    let v = c
        .with_options(RequestOptions::new().header("X-Request-Id", "trace-1"))
        .request(Method::GET, "/campaigns-overview", &[], None)
        .await
        .unwrap();
    assert_eq!(v["total"], 3);
    let v = c
        .with_options(RequestOptions::new().idempotency_key("del-1"))
        .request(Method::DELETE, "suppressions/abc", &[], None)
        .await
        .unwrap();
    assert!(v.is_null());
    let rows: Vec<Value> = c
        .paginate(Method::GET, "/emails", &[("limit", "50")], None)
        .try_collect()
        .await
        .unwrap();
    assert!(!rows.is_empty());
}

#[tokio::test]
async fn unknown_fields_and_nulls_are_tolerated() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user_id": "u", "email": null, "scopes": null, "brand_new_field": {"x": 1}
        })))
        .mount(&server)
        .await;
    let me = client(&server).me().get().await.unwrap();
    assert_eq!(me.email, "");
    assert!(me.scopes.is_empty());
    assert_eq!(me.extra["brand_new_field"]["x"], 1);
}

#[tokio::test]
async fn sends_one_authorization_header_only() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(|req: &Request| {
            let n = req.headers.get_all("authorization").iter().count();
            ResponseTemplate::new(if n == 1 { 200 } else { 400 }).set_body_json(fixture("me.json"))
        })
        .mount(&server)
        .await;
    client(&server).me().get().await.unwrap();
}
