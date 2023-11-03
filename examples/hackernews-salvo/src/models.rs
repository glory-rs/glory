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

#[cfg(feature = "web-ssr")]
pub fn show_story_api_url(id: usize) -> String {
    format!("https://node-hnapi.herokuapp.com/item/{}", id)
}
#[cfg(not(feature = "web-ssr"))]
pub fn show_story_api_url(id: usize) -> String {
    format!("/api/stories/{}", id)
}

#[cfg(feature = "web-ssr")]
pub fn list_stories_api_url(cate: impl AsRef<str>, page: usize) -> String {
    format!("https://node-hnapi.herokuapp.com/{}?page={}", cate.as_ref(), page)
}
#[cfg(not(feature = "web-ssr"))]
pub fn list_stories_api_url(cate: impl AsRef<str>, page: usize) -> String {
    format!("/api/stories/?cate={}&page={}", cate.as_ref(), page)
}

#[cfg(feature = "web-ssr")]
pub fn show_user_api_url(user_id: impl AsRef<str>) -> String {
    format!("https://hacker-news.firebaseio.com/v0/user/{}.json", user_id.as_ref())
}
#[cfg(not(feature = "web-ssr"))]
pub fn show_user_api_url(user_id: impl AsRef<str>) -> String {
    format!("/api/users/{}", user_id.as_ref())
}

#[cfg(not(feature = "web-ssr"))]
pub async fn fetch_api<T>(path: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    glory::info!("fetching {}", path);
    gloo_net::http::Request::get(path)
        .send()
        .await
        .map_err(|e| glory::error!("{e}"))
        .ok()?
        .json::<T>()
        .await
        .ok()
}
#[cfg(feature = "web-ssr")]
pub async fn fetch_api<T>(path: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    println!("fetching {}", path);
    reqwest::Client::new()
        .get(path)
        .send()
        .await
        .map_err(|e| tracing::error!("{e}"))
        .ok()?
        .json::<T>()
        .await
        .ok()
}
