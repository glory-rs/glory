use glory::reflow::*;
use glory::routing::*;
use glory::web::widgets::*;
use glory::widgets::*;
use glory::*;

use crate::models::*;

fn category(from: &str) -> &'static str {
    match from {
        "new" => "newest",
        "show" => "show",
        "ask" => "ask",
        "job" => "jobs",
        _ => "news",
    }
}

#[derive(Debug)]
pub struct ListStories {
    hide_more_link: Cage<bool>,
}
impl ListStories {
    pub fn new() -> Self {
        Self {
            hide_more_link: Cage::new(false),
        }
    }
}
impl Widget for ListStories {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "Stories".into());
        info.description.revise(|mut v| *v = "stories".into());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("ShowStory::build");
        let (page, story_type) = {
            let truck = ctx.truck();
            let locator = truck.obtain::<Locator>().unwrap();
            let page = locator
                .params()
                .get()
                .get("page")
                .and_then(|page| page.parse::<usize>().ok())
                .unwrap_or_default();
            (page, locator.params().get().get("type").cloned().unwrap_or("top".into()))
        };
        let api_url = format!("{}?page={}", category(&story_type), page);
        let loader = Loader::new(
            || async move {
                println!("fetchingssss {}", story_api_url(&api_url));
                fetch_api::<Vec<Story>>(story_api_url(&api_url).as_ref()).await 
            },
            |stories, ctx| {
                if let Some(stories) = stories {
                    ul().fill(Each::new(Cage::new(stories.clone()), |story| story.id, |story| ShowStory(story.clone())))
                        .show_in(ctx);
                } else {
                    h2().html("News not found").show_in(ctx);
                }
            },
        )
        .fallback(|ctx| {
            p().html("Loading story...").show_in(ctx);
        });

        div()
            .class("news-view")
            .fill(
                div()
                    .class("news-list-nav")
                    .fill(
                        span().fill(
                            Switch::new()
                                .case(Cage::new(page > 1), {
                                    let story_type = story_type.clone();
                                    move || {
                                        a().class("page-link")
                                            .href(format!("/{}?page={}", story_type, page - 1))
                                            .attr("aria-label", "Previous Page")
                                            .html("< prev")
                                    }
                                })
                                .case(Cage::new(true), || {
                                    span().class("page-link disabled").attr("aria-hidden", true).html("< prev")
                                }),
                        ),
                    )
                    .fill(span().html(format!("page {page}")))
                    .fill(
                        span()
                            .class("page-link")
                            .toggle_class("disabled", self.hide_more_link.clone())
                            .attr("aria-hidden", self.hide_more_link.clone())
                            .fill(
                                a().href(format!("/{}?page={}", story_type, page + 1))
                                    .attr("aria-label", "Next Page")
                                    .html("more >"),
                            ),
                    ),
            )
            .fill(main_().class("news-list").fill(div().fill(loader)))
            .show_in(ctx);
    }
}

#[derive(Debug)]
pub struct ShowStory(Story);
impl ShowStory {
    pub fn new(story: Story) -> Self {
        Self(story)
    }
}
impl Widget for ShowStory {
    fn build(&mut self, ctx: &mut Scope) {
        let story = &self.0;

        li().class("news-item")
            .fill(span().class("score").html(story.points.to_string()))
            .fill(span().class("title").then(|title| {
                if !story.url.starts_with("item?id=") {
                    title.fill(
                        span()
                            .fill(a().href(story.url.clone()).target("_blank").rel("noreferrer").text(story.title.clone()))
                            .fill(span().class("host").html(format!("({})", story.domain))),
                    )
                } else {
                    title.fill(a().href(format!("/stories/{}", story.id)).text(story.title.clone()))
                }
            }))
            .fill(br())
            .fill(span().class("meta").then(|meta| {
                if story.story_type != "job" {
                    meta.fill(
                        span()
                            .fill("by ")
                            .fill(story.user.clone().map(|user| a().href(format!("/users/{}", user)).text(user)))
                            .fill(format!(" {} | ", story.time_ago))
                            .fill(a().href(format!("/stories/{}", story.id)).text(if story.comments_count > 0 {
                                format!("{} comments", story.comments_count)
                            } else {
                                "discuss".into()
                            })),
                    )
                } else {
                    meta.fill(a().href(format!("/item/{}", story.id)).text(story.title.clone()))
                }
            }))
            .then(|meta| {
                if story.story_type != "link" {
                    meta.fill(" ").fill(span().class("label").html(story.story_type.clone()))
                } else {
                    meta
                }
            })
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
