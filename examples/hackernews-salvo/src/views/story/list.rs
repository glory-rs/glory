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
        info!("ListStories::new");
        Self {
            hide_more_link: Cage::new(false),
        }
    }
}
impl Widget for ListStories {
    fn attach(&mut self, ctx: &mut Scope) {
        info!("ListStories::attach");
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "Stories".into());
        info.description.revise(|mut v| *v = "stories".into());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("ListStories::build");
        let (page, story_type) = {
            let truck = ctx.truck();
            let locator = truck.obtain::<Locator>().unwrap();
            let queries = locator.queries();
            let page = Bond::new(move || queries.clone().get().get("page").and_then(|page| page.parse::<usize>().ok()).unwrap_or(1));
            let params = locator.params();
            let story_type = Bond::new(move || params.get().get("*?story_type").cloned().unwrap_or("top".into()));
            (page, story_type)
        };
        let loader = Loader::new(
            {
                let page = page.clone();
                let story_type = story_type.clone();
                move || {
                    let cate = category(&*story_type.clone().get());
                    let page = *page.clone().get();
                    async move { fetch_api::<Vec<Story>>(list_stories_api_url(cate, page).as_ref()).await }
                }
            },
            |stories, ctx| {
                if let Some(stories) = stories {
                    ul().fill(Each::new(
                        Lotus::<Vec<Story>>::from(stories.clone()),
                        |story: &Story| story.id,
                        |story| ShowStory::new(story.clone()),
                    ))
                    .show_in(ctx);
                } else {
                    h2().html("News not found").show_in(ctx);
                }
            },
        )
        .fallback(|ctx| {
            div().class("loading").fill(p().html("Loading stories...")).show_in(ctx);
        });

        div()
            .class("news-view")
            .fill(
                div()
                    .class("news-list-nav")
                    .fill(
                        span().fill(
                            Switch::new()
                                .case(page.map(|v| *v > 1), {
                                    let href = Bond::new({
                                        let story_type = story_type.clone();
                                        let page = page.clone();
                                        move || format!("/{}?page={}", *story_type.clone().get(), *page.clone().get() - 1)
                                    });
                                    move || {
                                        a().class("page-link")
                                            .href(href.clone())
                                            .attr("aria-label", "Previous Page")
                                            .html("< prev")
                                    }
                                })
                                .case(Cage::new(true), || {
                                    span().class("page-link disabled").attr("aria-hidden", "true").html("< prev")
                                }),
                        ),
                    )
                    .fill(span().html(page.map(|page| format!("page {}", page))))
                    .fill(
                        span()
                            .class("page-link")
                            .toggle_class("disabled", self.hide_more_link.clone())
                            .attr("aria-hidden", self.hide_more_link.map(|v|v.to_string()))
                            .fill(
                                a().href(Bond::new({
                                    let story_type = story_type.clone();
                                    let page = page.clone();
                                    move || format!("/{}?page={}", *story_type.get(), *page.get() + 1)
                                }))
                                .attr("aria-label", "Next Page")
                                .html("more >"),
                            ),
                    ),
            )
            .fill(main().class("news-list").fill(div().fill(loader)))
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
            .fill(
                span().class("meta").fill(
                    Switch::new()
                        .case(Cage::new(story.story_type != "job"), {
                            let story = story.clone();
                            move || {
                                span()
                                    .fill("by ")
                                    .fill(story.user.clone().map(|user| a().href(format!("/users/{}", user)).text(user)))
                                    .fill(format!(" {} | ", story.time_ago))
                                    .fill(a().href(format!("/stories/{}", story.id)).text(if story.comments_count > 0 {
                                        format!("{} comments", story.comments_count)
                                    } else {
                                        "discuss".into()
                                    }))
                            }
                        })
                        .case(Cage::new(true), {
                            let story = story.clone();
                            move || a().href(format!("/item/{}", story.id)).text(story.title.clone())
                        }),
                ),
            )
            .fill(Switch::new().case(Cage::new(story.story_type != "link"), {
                let story = story.clone();
                move || span().class("label").html(story.story_type.clone())
            }))
            .show_in(ctx);
    }
}
