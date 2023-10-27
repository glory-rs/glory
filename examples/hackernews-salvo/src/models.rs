use glory::Cage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct PageInfo {
    pub title: Cage<String>,
    pub description: Cage<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Story {
    pub id: usize,
    pub title: String,
    pub points: Option<i32>,
    pub user: Option<String>,
    pub time: usize,
    pub time_ago: String,
    #[serde(alias = "type")]
    pub story_type: String,
    pub url: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub comments: Vec<Comment>,
    pub comments_count: Option<usize>,
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

pub fn story_api_url(path: &str) -> String {
    format!("https://node-hnapi.herokuapp.com/{path}")
}

pub fn user_api_url(path: &str) -> String {
    format!("https://hacker-news.firebaseio.com/v0/user/{path}.json")
}

#[cfg(not(feature = "web-ssr"))]
pub async fn fetch_api<T>(path: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let json = gloo_net::http::Request::get(path)
        .abort_signal(abort_signal.as_ref())
        .send()
        .await
        .map_err(|e| glory::error!("{e}"))
        .ok()?
        .text()
        .await
        .ok()?;

    serde_json::from_str(&json).ok()
}
#[cfg(feature = "web-ssr")]
pub async fn fetch_api<T>(path: &str) -> Option<T>
where
    T: Serialize,
{
    let json = reqwest::get(path).await.map_err(|e| tracing::error!("{e}")).ok()?.text().await.ok()?;
    T::de(&json).map_err(|e| tracing::error!("{e}")).ok()
}
