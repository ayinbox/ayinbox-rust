//! Response and request types.
//!
//! Response types ignore fields they do not know and keep them in `extra`. A missing or
//! `null` field becomes its default. Fields the API has not pinned down are `serde_json::Value`.
//! Timestamps are RFC 3339 strings.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

pub(crate) fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

macro_rules! model {
    (
        $(#[$m:meta])*
        pub struct $name:ident {
            $( $(#[$fm:meta])* pub $f:ident : $t:ty ),* $(,)?
        }
    ) => {
        $(#[$m])*
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[non_exhaustive]
        pub struct $name {
            $(
                $(#[$fm])*
                #[serde(default, deserialize_with = "lenient")]
                pub $f: $t,
            )*
            /// Fields this version of the library does not model.
            #[serde(flatten)]
            pub extra: Map<String, Value>,
        }
    };
}

model! {
    /// The caller's identity (`GET /me`).
    pub struct Identity {
        /// User id.
        pub user_id: String,
        /// Email address.
        pub email: String,
        /// First and last name joined; may be empty.
        pub name: String,
        /// First name.
        pub first_name: String,
        /// Last name.
        pub last_name: String,
        /// Workspace id.
        pub organization_id: Option<String>,
        /// Workspace name.
        pub organization_name: Option<String>,
        /// `api_key`, `oauth` or `jwt`.
        pub auth_type: String,
        /// Lowercased permission names, sorted.
        pub scopes: Vec<String>,
    }
}

model! {
    /// A campaign sender.
    pub struct CampaignSender {
        /// Mailbox id.
        pub email_account_id: String,
        /// Rotation weight.
        pub weight: i64,
        /// Whether the sender is used.
        pub enabled: bool,
        /// Last send from this sender.
        pub last_sent_at: Option<String>,
    }
}

model! {
    /// A campaign.
    pub struct Campaign {
        /// Campaign id.
        pub id: String,
        /// Creator (attribution only).
        pub user_id: String,
        /// Workspace id.
        pub organization_id: Option<String>,
        /// Name.
        pub name: String,
        /// Description.
        pub description: String,
        /// `draft`, `active`, `paused`, `completed`, `paused_undeliverable`, `paused_guardrail`,
        /// `paused_no_accounts`, or a value added later.
        pub status: String,
        /// Stop the sequence for a lead who replies.
        pub stop_on_reply: bool,
        /// Stop the sequence for a lead who books a meeting.
        pub stop_on_meeting: bool,
        /// Open tracking.
        pub open_tracking: bool,
        /// Link tracking.
        pub link_tracking: bool,
        /// Send plain text only.
        pub text_only: bool,
        /// Add a List-Unsubscribe header.
        pub unsubscribe_header: bool,
        /// Send to risky addresses.
        pub risky_emails: bool,
        /// Emails per mailbox per day.
        pub daily_limit: i64,
        /// `inherit`, `text`, `link` or `off`.
        pub unsubscribe_mode: String,
        /// CC addresses.
        pub cc: Vec<String>,
        /// BCC addresses.
        pub bcc: Vec<String>,
        /// Start date.
        pub start_date: Option<String>,
        /// End date.
        pub end_date: Option<String>,
        /// IANA zone; empty follows the workspace.
        pub timezone: String,
        /// The resolved zone.
        pub effective_timezone: String,
        /// Weekday bitmask (legacy schedule).
        pub days: i64,
        /// `HH:MM` (legacy schedule).
        pub start_time: String,
        /// `HH:MM` (legacy schedule).
        pub end_time: String,
        /// Seven arrays of `{start, end}` minute windows, Sunday first.
        pub schedule_windows: Value,
        /// Mailbox tag ids.
        pub email_tags: Vec<String>,
        /// Folder ids.
        pub folders: Vec<String>,
        /// Lead ordering field.
        pub contact_order_by: String,
        /// Lead ordering direction.
        pub contact_order_dir: String,
        /// `tags` or `explicit`.
        pub sender_strategy: String,
        /// Sender rotation mode.
        pub rotation_mode: String,
        /// Explicit senders.
        pub senders: Vec<CampaignSender>,
        /// Reply-paced ramp on.
        pub ramp_enabled: bool,
        /// `off`, `prefer` or `strict`.
        pub esp_match_mode: String,
        /// New leads per day; 0 is unlimited.
        pub max_new_leads_per_day: i64,
        /// Keep running and wait for new leads.
        pub continuous: bool,
        /// Hold first emails for review.
        pub review_first_emails: bool,
        /// Minutes before a new lead's first email.
        pub entry_delay_minutes: i64,
        /// Set while an active campaign waits for leads.
        pub idle_since: Option<String>,
        /// Guardrail on.
        pub guardrail_enabled: bool,
        /// Tracking domain.
        pub tracking_domain: String,
        /// Last status change.
        pub last_status_change_at: Option<String>,
        /// Created at.
        pub created_at: String,
        /// Updated at.
        pub updated_at: String,
    }
}

model! {
    /// Result of pausing a campaign.
    pub struct CampaignPauseResult {
        /// `stopped`.
        pub status: String,
    }
}

model! {
    /// A label on a contact.
    pub struct Category {
        /// Label id.
        pub id: String,
        /// Label title.
        pub title: String,
        /// Label colour.
        pub color: String,
    }
}

model! {
    /// A campaign a contact belongs to.
    pub struct ContactCampaign {
        /// Campaign id.
        pub id: String,
        /// Campaign name.
        pub name: String,
    }
}

model! {
    /// A contact. `GET /contacts/{id}` adds more fields (engagement, source and others), kept in `extra`.
    pub struct Contact {
        /// Contact id.
        pub id: String,
        /// First name.
        pub first_name: String,
        /// Last name.
        pub last_name: String,
        /// Email address.
        pub email: String,
        /// Company.
        pub company: String,
        /// Phone.
        pub phone: String,
        /// Custom fields.
        pub custom_fields: BTreeMap<String, String>,
        /// Whether the contact is subscribed.
        pub subscribed: bool,
        /// Campaigns the contact is in.
        pub campaigns: Vec<ContactCampaign>,
        /// Labels.
        pub categories: Vec<Category>,
        /// `valid`, `risky`, `invalid` or `unknown`.
        pub verification_status: String,
        /// Verification reason.
        pub verification_reason: String,
        /// Catch-all domain.
        pub is_catch_all: bool,
        /// Mail host.
        pub mail_host: String,
        /// Mailbox provider.
        pub esp_provider: String,
        /// Lead score (fields not pinned down).
        pub lead_score: Value,
        /// Progress in a campaign, on a search with one campaign id (fields not pinned down).
        pub campaign_lead: Value,
        /// Created at.
        pub created_at: String,
        /// Updated at.
        pub updated_at: String,
    }
}

model! {
    /// A connected mailbox. Many more fields are kept in `extra`.
    pub struct Mailbox {
        /// Mailbox id.
        pub id: String,
        /// Who connected it (attribution only).
        pub user_id: String,
        /// Workspace id.
        pub organization_id: Option<String>,
        /// Email address.
        pub email: String,
        /// Display name.
        pub name: String,
        /// `gmail`, `outlook` or `smtp_imap`.
        pub provider: String,
        /// Status.
        pub status: String,
        /// Mail host.
        pub mail_host: String,
        /// Last sync.
        pub last_synced_at: Option<String>,
        /// Cold emails per day.
        pub campaign_limit: i64,
        /// Seconds between sends.
        pub min_wait_time: i64,
        /// Set while warmup is on.
        pub warmup: Option<String>,
        /// Tags.
        pub tags: Vec<String>,
        /// Created at.
        pub created_at: String,
        /// Updated at.
        pub updated_at: String,
    }
}

model! {
    /// A message row in the Unibox list or a thread.
    pub struct MessagePreview {
        /// Message id.
        pub id: String,
        /// Mailbox id.
        pub email_id: String,
        /// Provider thread id (not a uuid).
        pub thread_id: String,
        /// From.
        pub from_addr: Vec<String>,
        /// To.
        pub to_addr: Vec<String>,
        /// Subject.
        pub subject: String,
        /// Snippet.
        pub snippet: String,
        /// Received at.
        pub internal_date: String,
        /// Read.
        pub seen: bool,
        /// Messages in the thread (list reads).
        pub message_count: i64,
        /// Thread has unread mail.
        pub has_unread: bool,
        /// Labels.
        pub labels: Vec<Category>,
        /// Mailbox the thread answers (thread reads).
        pub answers_mailbox_id: Option<String>,
        /// Campaign (list reads).
        pub campaign_id: Option<String>,
    }
}

/// Query for `GET /campaigns`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CampaignListParams {
    /// Name search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// `draft`, `active`, `paused` or `completed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Folder id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Page size; 1 to 100 recommended (default 50).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Cursor from a previous page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// A step inside `POST /campaigns`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StepInput {
    /// Step name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Subject.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Plain text body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_plain: Option<String>,
    /// HTML body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_html: Option<String>,
    /// Days to wait after this step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_after: Option<i64>,
    /// Send in the same thread (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_reply: Option<bool>,
    /// Any other step field.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Body for `POST /campaigns`. Only `name` is required. Creating never sends mail.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CampaignCreate {
    /// Name.
    pub name: String,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Emails per mailbox per day (3 to 5000, default 50).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_limit: Option<i64>,
    /// IANA zone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Stop for a lead who replies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_on_reply: Option<bool>,
    /// Open tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_tracking: Option<bool>,
    /// Link tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_tracking: Option<bool>,
    /// Plain text only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_only: Option<bool>,
    /// `tags` or `explicit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_strategy: Option<String>,
    /// Mailbox tag ids (create uses `email_tag_ids`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_tag_ids: Option<Vec<String>>,
    /// Folder ids (create uses `folder_ids`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_ids: Option<Vec<String>>,
    /// Explicit senders: objects with `email_account_id`, optional `weight` and `enabled`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub senders: Option<Vec<Value>>,
    /// Start date (RFC 3339).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// End date (RFC 3339).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Steps, created and connected in order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<StepInput>>,
    /// Any other field from the API reference.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One contact for `POST /contacts`. Only `email` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ContactInput {
    /// Email address.
    pub email: String,
    /// First name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    /// Last name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    /// Company.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    /// Phone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    /// Campaign ids. Adding a contact to an active campaign can send mail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaigns: Option<Vec<String>>,
    /// Label ids.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    /// Segment ids.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<String>>,
    /// Custom fields, merged key by key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<BTreeMap<String, String>>,
    /// Subscribed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribed: Option<bool>,
    /// Any other field from the API reference.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Body for `PATCH /contacts/{id}`. Omitted fields are left unchanged.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ContactUpdate {
    /// First name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    /// Last name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    /// Email address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Company.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    /// Phone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    /// Custom fields (replaces).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<BTreeMap<String, String>>,
    /// Subscribed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribed: Option<bool>,
    /// Campaign ids (replaces the list). Can send mail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaigns: Option<Vec<String>>,
    /// Label ids (replaces).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    /// Label ids to add.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_categories: Option<Vec<String>>,
    /// Label ids to remove.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_categories: Option<Vec<String>>,
    /// Any other field from the API reference.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Search for `POST /contacts/search`. `limit`, `cursor` and `category` go in the query;
/// everything else is the body, sent again unchanged on every page.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ContactListParams {
    /// Page size; 1 to 100 recommended (default 50).
    #[serde(skip)]
    pub limit: Option<u32>,
    /// Cursor from a previous page.
    #[serde(skip)]
    pub cursor: Option<String>,
    /// One label id.
    #[serde(skip)]
    pub category: Option<String>,
    /// Text search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Contacts in ALL of these campaigns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_ids: Option<Vec<String>>,
    /// Lead status (needs exactly one campaign id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead_status: Option<String>,
    /// Labels, ALL of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_ids: Option<Vec<String>>,
    /// Subscribed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribed: Option<bool>,
    /// Verification status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<String>,
    /// Sort field (default `created_at`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
    /// Ascending when true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse: Option<bool>,
    /// Any other search field from the API reference.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Query for `GET /emails`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MailboxListParams {
    /// Search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// Tag id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Page size; 1 to 100 recommended (default 50).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Cursor from a previous page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Query for `GET /unibox`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ThreadListParams {
    /// Page size, 1 to 100 (default 50).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Cursor from a previous page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// `inbox`, `sent`, `drafts`, `archive`, `spam` or `trash`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// `sent` or `received`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Only unseen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unseen: Option<bool>,
    /// Campaign id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<String>,
    /// Contact id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<String>,
    /// Mailbox id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_id: Option<String>,
    /// Any other filter from the API reference (`from`, `subject`, `since`, ...).
    #[serde(flatten)]
    pub extra: BTreeMap<String, String>,
}

/// Query for `GET /unibox/thread` besides the thread id.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ThreadGetParams {
    /// Mailbox id; omit to read across every mailbox.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_id: Option<String>,
    /// Page size, 1 to 500 (default 500).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Cursor from a previous page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}
