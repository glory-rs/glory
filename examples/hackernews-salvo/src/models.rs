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
    #[serde(default)]
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
    pub comments: Vec<Comment>,
    #[serde(default)]
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

pub fn story_api_url(path: impl AsRef<str>) -> String {
    format!("https://node-hnapi.herokuapp.com/{}", path.as_ref())
}

pub fn user_api_url(user_id: usize) -> String {
    format!("https://hacker-news.firebaseio.com/v0/user/{user_id}.json")
}

#[cfg(not(feature = "web-ssr"))]
pub async fn fetch_api<T>(path: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let json = gloo_net::http::Request::get(path)
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
T: serde::de::DeserializeOwned,
{
    let json = reqwest::get(path).await.map_err(|e| tracing::error!("{e}")).ok()?.text().await.ok()?;
    serde_json::from_str(&json).map_err(|e| tracing::error!("{e}")).ok()
}
