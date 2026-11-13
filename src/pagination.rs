//! Cursor pagination: one page at a time, or every row as a stream.

use std::collections::VecDeque;
use std::pin::Pin;

use futures_util::stream::{self, Stream};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::client::Client;
use crate::error::Result;

/// A stream of rows across every page. Each item is a row or the error that ended the walk.
pub type PageStream<T> = Pin<Box<dyn Stream<Item = Result<T>> + Send>>;

/// The `pagination` object of a list response.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Pagination {
    /// Opaque cursor for the next page, or `None` on the last page. Pass it back unchanged.
    pub next_cursor: Option<String>,
    /// Whether another page exists.
    pub has_more: bool,
    /// Total rows, on the endpoints that report it. Never rely on it.
    pub total: Option<i64>,
}

/// One page of a list response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Page<T> {
    /// The rows on this page.
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
    /// Cursor details.
    #[serde(default)]
    pub pagination: Pagination,
    /// Any other top-level fields (for example `counts` and `lead_counts` on a contact search).
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl<T> Page<T> {
    /// The cursor for the next page, when there is one.
    pub fn next_cursor(&self) -> Option<&str> {
        if self.pagination.has_more {
            self.pagination
                .next_cursor
                .as_deref()
                .filter(|c| !c.is_empty())
        } else {
            None
        }
    }
}

struct State<T> {
    client: Client,
    method: Method,
    path: String,
    query: Vec<(String, String)>,
    body: Option<Value>,
    cursor: Option<String>,
    buffer: VecDeque<T>,
    done: bool,
}

pub(crate) fn paginate<T>(
    client: Client,
    method: Method,
    path: String,
    query: Vec<(String, String)>,
    body: Option<Value>,
) -> PageStream<T>
where
    T: DeserializeOwned + Send + 'static,
{
    let query = query.into_iter().filter(|(k, _)| k != "cursor").collect();
    let state = State {
        client,
        method,
        path,
        query,
        body,
        cursor: None,
        buffer: VecDeque::new(),
        done: false,
    };
    Box::pin(stream::unfold(state, |mut st| async move {
        loop {
            if let Some(row) = st.buffer.pop_front() {
                return Some((Ok(row), st));
            }
            if st.done {
                return None;
            }
            let mut query = st.query.clone();
            if let Some(c) = &st.cursor {
                query.push(("cursor".to_owned(), c.clone()));
            }
            let page: Result<Page<T>> = st
                .client
                .call(st.method.clone(), &st.path, &query, st.body.as_ref(), true)
                .await;
            match page {
                Err(e) => {
                    st.done = true;
                    return Some((Err(e), st));
                }
                Ok(page) => {
                    match page.next_cursor() {
                        Some(c) => st.cursor = Some(c.to_owned()),
                        None => st.done = true,
                    }
                    st.buffer.extend(page.data);
                }
            }
        }
    }))
}
