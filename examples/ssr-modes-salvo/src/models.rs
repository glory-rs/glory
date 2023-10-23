use serde::{Deserialize, Serialize};
use glory::Cage;

#[derive(Clone, Debug, Default)]
pub struct PageInfo {
    pub title: Cage<String>,
    pub description: Cage<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub id: usize,
    pub title: String,
    pub description: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostMetadata {
    pub id: usize,
    pub title: String,
}