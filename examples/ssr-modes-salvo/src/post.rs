use once_cell::sync::Lazy;

use crate::models::{Post, PostMetadata};

static POSTS: Lazy<Vec<Post>> = Lazy::new(|| {
    vec![
        Post {
            id: 0,
            title: "My first post".to_string(),
            description: "This is my first post".to_string(),
            content: r#"<p>This is my first post</p><p>This is my first post</p>
            <p>A language empowering everyone
            to build reliable and efficient software. </p>
            <p>Rust is blazingly fast and memory-efficient: with no runtime or garbage collector, 
            it can powwer performance-critical services, run on embedded devices, and easily 
            integrate with other languages.</p>"#.to_string(),
        },
        Post {
            id: 1,
            title: "My second post".to_string(),
            description: "This is my first post".to_string(),
            content: r#"<p>This is my second post</p><p>This is my second post</p>
            <p>Rust’s rich type system and ownership model guarantee memory-safety and thread-safety — enabling you to eliminate many classes of bugs at compile-time. </p>
            <p>A type that is constructed with a Promise and can then be </p>"#.to_string(),
        },
        Post {
            id: 2,
            title: "My third post".to_string(),
            description: "This is my first post".to_string(),
            content: r#"<p>This is my third post</p><p>This is my third post</p>
            <p>Rust has great documentation, a friendly compiler with useful error messages, and top-notch tooling — an integrated package manager and build tool, 
            smart multi-editor support with auto-completion and type inspections, an auto-formatter, and more. </p>
            <p>This is my third pozzst</p>"#.to_string(),
        },
    ]
});

pub fn list_posts() -> Vec<PostMetadata> {
    POSTS
        .iter()
        .map(|post| PostMetadata {
            id: post.id,
            title: post.title.clone(),
        })
        .collect()
}
pub fn get_post(id: usize) -> Option<Post> {
    POSTS.iter().find(|post| post.id == id).cloned()
}
