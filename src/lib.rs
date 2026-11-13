//! Official Rust client for the [AY Inbox](https://ayinbox.com) REST API.
//!
//! AY Inbox is a hosted service. This crate is a client of its public API at
//! `https://api.ayinbox.com/v1`.
//!
//! ```no_run
//! use ayinbox::{Client, CampaignListParams};
//! use futures_util::TryStreamExt;
//!
//! # async fn run() -> Result<(), ayinbox::Error> {
//! let client = Client::from_env()?; // reads AYINBOX_API_KEY
//! let me = client.me().get().await?;
//! println!("signed in as {}", me.email);
//!
//! let mut campaigns = client.campaigns().list_all(&CampaignListParams::default());
//! while let Some(c) = campaigns.try_next().await? {
//!     println!("{}: {}", c.name, c.status);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Every route not wrapped by a method is reachable with [`Client::request`] and
//! [`Client::paginate`]. See <https://docs.ayinbox.com/api/>.

#![deny(missing_docs)]

mod client;
mod error;
mod pagination;
mod resources;
mod types;
pub mod webhooks;

pub use client::{
    Client, ClientBuilder, RequestOptions, DEFAULT_BASE_URL, DEFAULT_MAX_RETRIES, DEFAULT_TIMEOUT,
};
pub use error::{ApiError, Error, Result};
pub use pagination::{Page, PageStream, Pagination};
pub use reqwest::Method;
pub use resources::{Campaigns, Contacts, Mailboxes, Me, Unibox};
pub use types::{
    Campaign, CampaignCreate, CampaignListParams, CampaignPauseResult, CampaignSender, Category,
    Contact, ContactCampaign, ContactInput, ContactListParams, ContactUpdate, Identity, Mailbox,
    MailboxListParams, MessagePreview, StepInput, ThreadGetParams, ThreadListParams,
};
pub use webhooks::{WebhookError, WebhookEvent};
