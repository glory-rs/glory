use glory::Cage;
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

#[derive(Clone, Debug, Default)]
pub struct PageInfo {
    pub title: Cage<String>,
    pub description: Cage<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Story {
    pub id: usize,
    pub title: String,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub points: i32,
    pub user: Option<String>,
    pub time: usize,
    pub time_ago: String,
    #[serde(alias = "type")]
    pub story_type: String,
    pub url: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub comments: Vec<Comment>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub comments_count: usize,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Comment {
    pub id: usize,
    pub level: usize,
    pub user: Option<String>,
    pub time: usize,
    pub time_ago: String,
    pub content: Option<String>,
    pub comments: Vec<Comment>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct User {
    pub created: usize,
    pub id: String,
    pub karma: i32,
    pub about: Option<String>,
}

use glory::serverfn::ServerFnError;

// Server functions: the bodies below run on the server only (they call
// the upstream HN API); wasm builds get generated stubs that POST to
// `/__glory/fn/<name>`, mounted in main.rs via
// `glory::serverfn::salvo_mount::router()`. This replaces the old
// hand-written `/api/...` salvo routes, the cfg'd URL builders and the
// dual gloo-net/reqwest fetch implementations.

#[glory::server]
pub async fn fetch_user(id: String) -> Result<Option<User>, ServerFnError> {
    let url = format!("https://hacker-news.firebaseio.com/v0/user/{id}.json");
    Ok(upstream_json::<User>(&url).await)
}

#[glory::server]
pub async fn fetch_stories(cate: String, page: usize) -> Result<Vec<Story>, ServerFnError> {
    let url = format!("https://node-hnapi.herokuapp.com/{cate}?page={page}");
    Ok(upstream_json::<Vec<Story>>(&url).await.unwrap_or_default())
}

#[glory::server]
pub async fn fetch_story(id: usize) -> Result<Option<Story>, ServerFnError> {
    let url = format!("https://node-hnapi.herokuapp.com/item/{id}");
    Ok(upstream_json::<Story>(&url).await)
}

/// Upstream HN API call — server-side helper, never compiled to wasm.
#[cfg(not(target_arch = "wasm32"))]
async fn upstream_json<T>(url: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    cfg_if::cfg_if! {
        if #[cfg(feature = "web-ssr")] {
            tracing::info!("fetching {url}");
            reqwest::Client::new()
                .get(url)
                .send()
                .await
                .map_err(|e| tracing::error!("{e}"))
                .ok()?
                .json::<T>()
                .await
                .map_err(|e| tracing::error!("{e}"))
                .ok()
        } else {
            // Feature-less check builds have no HTTP client; the binary is
            // only ever run with `web-ssr` enabled.
            let _ = url;
            None
        }
    }
}
