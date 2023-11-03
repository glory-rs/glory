use glory::reflow::*;
use glory::routing::*;
use glory::web::events;
use glory::web::widgets::*;
use glory::widgets::*;
use glory::*;

use crate::models::*;

#[derive(Debug)]
pub struct ShowStory;
impl Widget for ShowStory {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "Story".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("ShowStory::build");
        let story_id: usize = {
            let truck = ctx.truck();
            let locator = truck.obtain::<Locator>().unwrap();
            if let Some(id) = locator.params().get().get("id") {
                id.parse().unwrap_or_default()
            } else {
                0
            }
        };
        let info = {
            let truck = ctx.truck();
            truck.obtain::<PageInfo>().unwrap().clone()
        };
        let loader = Loader::new(
            move || async move {
                let url = show_story_api_url(story_id);
                fetch_api::<Story>(&url).await
            },
            move |story, ctx| {
                if let Some(story) = story {
                    info.title.revise(|mut v| *v = story.title.clone());
                    info.description.revise(|mut v| *v = story.title.clone());
                    div()
                        .class("item-view-header")
                        .fill(a().href(story.url.clone()).target("_blank").fill(h1().html(story.title.clone())))
                        .fill(story.user.clone().map(|user| {
                            p().class("meta")
                                .fill(story.points.to_string())
                                .fill(" points | by ")
                                .fill(a().href(format!("/users/{user}")).html(user.clone()))
                                .fill(format!(" {}", story.time_ago))
                        }))
                        .show_in(ctx);
                    div()
                        .class("item-view-comments")
                        .fill(
                            p().class("item-view-comments-header")
                                .fill(
                                    Switch::new()
                                        .case(Cage::new(story.comments_count > 0), {
                                            let story = story.clone();
                                            move || span().html(format!("{} comments", story.comments_count))
                                        })
                                        .case(Cage::new(true), || span().html("No comments yet.")),
                                )
                                .fill(ul().class("comment-children").fill(Each::new(
                                    Cage::new(story.comments.clone()),
                                    |comment| comment.id,
                                    |comment| ShowComment::new(comment.clone()),
                                ))),
                        )
                        .show_in(ctx);
                } else {
                    h2().html("Story not found").show_in(ctx);
                }
            },
        )
        .fallback(|ctx| {
            div().class("loading").fill(p().html("Loading story...")).show_in(ctx);
        });

        div().class("user-view").fill(loader).show_in(ctx);
    }
}

#[derive(Debug, Clone)]
pub struct ShowComment {
    comment: Comment,
    opened: Cage<bool>,
}
impl ShowComment {
    pub fn new(comment: Comment) -> Self {
        Self {
            comment,
            opened: Cage::new(false),
        }
    }
}
impl Widget for ShowComment {
    fn build(&mut self, ctx: &mut Scope) {
        let comment = &self.comment;
        let opened = self.opened.clone();
        li().class("comment")
            .fill(
                div()
                    .class("by")
                    .fill(
                        a().href(format!("/users/{}", comment.user.clone().unwrap_or_default()))
                            .text(comment.user.clone()),
                    )
                    .fill(format!(" {}", comment.time_ago)),
            )
            .fill(div().class("text").html(comment.content.clone()))
            .fill(Switch::new().case(Cage::new(!comment.comments.is_empty()), {
                let opened = opened.clone();
                let comment = comment.clone();
                move || {
                    div().fill(
                        div()
                            .class("toggle")
                            .toggle_class("open", opened.clone())
                            .fill(
                                a().on(events::click, {
                                    let opened = opened.clone();
                                    move |_| {
                                        opened.revise(|mut v| *v = !*v);
                                    }
                                })
                                .html(Bond::new({
                                    let opened = opened.clone();
                                    let len = comment.comments.len();
                                    move || {
                                        if *opened.get() {
                                            "[-]".to_owned()
                                        } else {
                                            format!("[+] {}{} collapsed", len, pluralize(len))
                                        }
                                    }
                                })),
                            )
                            .fill(Switch::new().case(opened.clone(), {
                                let comments = Cage::new(comment.comments.clone());
                                move || {
                                    ul().class("comment-children").fill(Each::new(
                                        comments.clone(),
                                        |comment| comment.id,
                                        |comment| ShowComment::new(comment.clone()),
                                    ))
                                }
                            })),
                    )
                }
            }))
            .show_in(ctx);
    }
}

fn pluralize(n: usize) -> &'static str {
    if n == 1 {
        " reply"
    } else {
        " replies"
    }
}
