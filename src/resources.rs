//! Resource handles returned by [`Client`](crate::Client).

use reqwest::Method;
use serde_json::Value;

use crate::client::{seg, to_query, Client};
use crate::error::{Error, Result};
use crate::pagination::{paginate, Page, PageStream};
use crate::types::*;

fn body<T: serde::Serialize>(v: &T) -> Result<Value> {
    serde_json::to_value(v).map_err(Error::Decode)
}

/// `GET /me`.
#[derive(Debug, Clone, Copy)]
pub struct Me<'a> {
    pub(crate) client: &'a Client,
}

impl Me<'_> {
    /// `GET /me`: validates the credential and returns who it acts for. Needs no scope.
    pub async fn get(&self) -> Result<Identity> {
        self.client.call(Method::GET, "/me", &[], None, false).await
    }
}

/// Campaigns.
#[derive(Debug, Clone, Copy)]
pub struct Campaigns<'a> {
    pub(crate) client: &'a Client,
}

impl Campaigns<'_> {
    /// `GET /campaigns`: one page. Scope `READ_CAMPAIGNS`.
    pub async fn list(&self, params: &CampaignListParams) -> Result<Page<Campaign>> {
        self.client
            .call(Method::GET, "/campaigns", &to_query(params), None, false)
            .await
    }

    /// `GET /campaigns`: every campaign, across pages.
    pub fn list_all(&self, params: &CampaignListParams) -> PageStream<Campaign> {
        paginate(
            self.client.clone(),
            Method::GET,
            "/campaigns".to_owned(),
            to_query(params),
            None,
        )
    }

    /// `GET /campaigns/{id}`. Scope `READ_CAMPAIGNS`.
    pub async fn get(&self, id: &str) -> Result<Campaign> {
        let path = format!("/campaigns/{}", seg(id));
        self.client.call(Method::GET, &path, &[], None, false).await
    }

    /// `POST /campaigns`: creates a draft. Never sends mail. Scope `WRITE_CAMPAIGNS`.
    pub async fn create(&self, params: &CampaignCreate) -> Result<Campaign> {
        let b = body(params)?;
        self.client
            .call(Method::POST, "/campaigns", &[], Some(&b), false)
            .await
    }

    /// `POST /campaigns/{id}/stop`: pauses an active campaign. Sends nothing.
    /// Scope `WRITE_CAMPAIGNS`, `START_CAMPAIGNS` or `SEND_CAMPAIGNS`.
    pub async fn pause(&self, id: &str) -> Result<CampaignPauseResult> {
        let path = format!("/campaigns/{}/stop", seg(id));
        self.client
            .call(Method::POST, &path, &[], None, false)
            .await
    }
}

/// Contacts.
#[derive(Debug, Clone, Copy)]
pub struct Contacts<'a> {
    pub(crate) client: &'a Client,
}

fn search_parts(params: &ContactListParams) -> Result<(Vec<(String, String)>, Value)> {
    let mut q = Vec::new();
    if let Some(l) = params.limit {
        q.push(("limit".to_owned(), l.to_string()));
    }
    if let Some(c) = &params.cursor {
        q.push(("cursor".to_owned(), c.clone()));
    }
    if let Some(c) = &params.category {
        q.push(("category".to_owned(), c.clone()));
    }
    Ok((q, body(params)?))
}

impl Contacts<'_> {
    /// `POST /contacts/search`: one page. A read, retried like a `GET`. Scope `READ_CONTACTS`.
    pub async fn list(&self, params: &ContactListParams) -> Result<Page<Contact>> {
        let (q, b) = search_parts(params)?;
        self.client
            .call(Method::POST, "/contacts/search", &q, Some(&b), true)
            .await
    }

    /// `POST /contacts/search`: every match, across pages, with the same body on every page.
    pub fn list_all(&self, params: &ContactListParams) -> PageStream<Contact> {
        let (q, b) = match search_parts(params) {
            Ok(parts) => parts,
            Err(e) => return Box::pin(futures_util::stream::once(async move { Err(e) })),
        };
        paginate(
            self.client.clone(),
            Method::POST,
            "/contacts/search".to_owned(),
            q,
            Some(b),
        )
    }

    /// `GET /contacts/{id}`: the contact with its detail fields (in `extra`). Scope `READ_CONTACTS`.
    pub async fn get(&self, id: &str) -> Result<Contact> {
        let path = format!("/contacts/{}", seg(id));
        self.client.call(Method::GET, &path, &[], None, false).await
    }

    /// `POST /contacts`: creates or updates (by address) up to 10,240 contacts.
    /// Adding contacts to an active campaign can send mail. Scope `WRITE_CONTACTS`.
    pub async fn create(&self, contacts: &[ContactInput]) -> Result<Vec<Contact>> {
        let b = body(&contacts)?;
        self.client
            .call(Method::POST, "/contacts", &[], Some(&b), false)
            .await
    }

    /// `PATCH /contacts/{id}`. Scope `WRITE_CONTACTS`.
    pub async fn update(&self, id: &str, params: &ContactUpdate) -> Result<Contact> {
        let path = format!("/contacts/{}", seg(id));
        let b = body(params)?;
        self.client
            .call(Method::PATCH, &path, &[], Some(&b), false)
            .await
    }

    /// `DELETE /contacts/{id}`. Scope `WRITE_CONTACTS`.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let path = format!("/contacts/{}", seg(id));
        self.client
            .send(Method::DELETE, &path, &[], None, false)
            .await
            .map(|_| ())
    }
}

/// Mailboxes (`/emails`).
#[derive(Debug, Clone, Copy)]
pub struct Mailboxes<'a> {
    pub(crate) client: &'a Client,
}

impl Mailboxes<'_> {
    /// `GET /emails`: one page. Scope `READ_EMAILS`.
    pub async fn list(&self, params: &MailboxListParams) -> Result<Page<Mailbox>> {
        self.client
            .call(Method::GET, "/emails", &to_query(params), None, false)
            .await
    }

    /// `GET /emails`: every mailbox, across pages.
    pub fn list_all(&self, params: &MailboxListParams) -> PageStream<Mailbox> {
        paginate(
            self.client.clone(),
            Method::GET,
            "/emails".to_owned(),
            to_query(params),
            None,
        )
    }
}

/// Unibox.
#[derive(Debug, Clone, Copy)]
pub struct Unibox<'a> {
    pub(crate) client: &'a Client,
}

impl Unibox<'_> {
    /// `GET /unibox`: one page of conversations, newest message each. Scope `READ_UNIBOX`.
    pub async fn list_threads(&self, params: &ThreadListParams) -> Result<Page<MessagePreview>> {
        self.client
            .call(Method::GET, "/unibox", &to_query(params), None, false)
            .await
    }

    /// `GET /unibox`: every conversation, across pages.
    pub fn list_threads_all(&self, params: &ThreadListParams) -> PageStream<MessagePreview> {
        paginate(
            self.client.clone(),
            Method::GET,
            "/unibox".to_owned(),
            to_query(params),
            None,
        )
    }

    /// `GET /unibox/thread`: one page of a conversation's messages (previews). Scope `READ_UNIBOX`.
    pub async fn get_thread(
        &self,
        thread_id: &str,
        params: &ThreadGetParams,
    ) -> Result<Page<MessagePreview>> {
        let mut q = vec![("thread_id".to_owned(), thread_id.to_owned())];
        q.extend(to_query(params));
        self.client
            .call(Method::GET, "/unibox/thread", &q, None, false)
            .await
    }
}
