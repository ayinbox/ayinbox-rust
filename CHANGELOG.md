# Changelog

## 0.1.0 (unreleased)

- First release.
- `Client` with a builder: API key from `AYINBOX_API_KEY`, base URL override (`AYINBOX_BASE_URL`), timeout, retries and an injectable `reqwest::Client`.
- Methods: `me().get`, `campaigns().list`, `get`, `create`, `pause`, `contacts().list`, `get`, `create`, `update`, `delete`, `mailboxes().list`, `unibox().list_threads`, `get_thread`.
- Generic `request` and `paginate`, `list_all` streams on every paginated list.
- Typed `Error` enum, retries with backoff that honour `Retry-After`, per-call idempotency keys through `with_options`.
- `webhooks::verify_signature` and `construct_event` with a constant-time compare.
