use std::cell::RefCell;
use std::rc::Rc;

use glory::reflow::*;
use glory::routing::*;
use glory::web::widgets::*;
use glory::widgets::*;
use glory::*;

fn category(from: &str) -> &'static str {
    match from {
        "new" => "newest",
        "show" => "show",
        "ask" => "ask",
        "job" => "jobs",
        _ => "news",
    }
}

pub struct ListStories{
    hide_more_link: Cage<bool>,
};
impl Widget for ListStories {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "Stories".into());
        info.description.revise(|mut v| *v = "stories".into());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("ShowStory::build");
        let (page: usize, story_type) = {
            let truck = ctx.truck();
            let locator = truck.obtain::<Locator>().unwrap();
            if let Some(page) = locator.params().get().get("page") {
                page.parse().unwrap_or_default()
            } else {
                0
            }
            (page,  locator.params().get().get("type").unwrap_or("top"))
        };
        let info = {
            let truck = ctx.truck();
            truck.obtain::<PageInfo>().unwrap().clone()
        };
        let api_url = format!("{}?page={}", category(&story_type), page);
        let loader = Loader::new(
            || models::fetch_api::<Vec<Story>>(&api::story_api_url(&api_url)).await,
            move |stories, ctx| {
                if let Some(stories) = stories {
                    ul().fill(Each::new(stories, |story|{
                        ShowStory(story.clone())
                    }))
                } else {
                    h2().html("News not found").show_in(ctx);
                }
            },
        )
        .fallback(|ctx| {
            p().html("Loading story...").show_in(ctx);
        });

        div().class("news-view").fill(div()
        .class("news-list-nav")
        .fill(span().fill(
            if page > 1 {
                a().class("page-link").href(format!("/{}?page={}", story_type, page - 1)).attr("aria-label", "Previous Page").html("< prev")
            } else {
                span().class("page-link disabled").attr("aria-hidden", true).html("< prev")
            }
        )).fill(
            span().html(format!("page {page}"))).fill(
                span().class("page-link")
                    .toggle_class("disabled", self.hide_more_link).attr("aria-hidden", self.hide_more_link)
                    .fill(a().href(format!("/{}?page={}", story_type, page + 1)).attr("aria-label", "Next Page").html("more >")))
        .show_in(ctx);

    main_().class("news-list").fill(
        div().fill(loader)
    )).show_in(ctx);
    }
}

pub struct ShowStory(Story)
impl ShowStory {
    pub fn new(story: Story) -> Self {
        Self(story)
    }
}
impl Widget for ShowStory {
    fn build(&mut self, ctx: &mut Scope) {
        li().class("news-item").fill(
        ).fill(
            span().class("score").html(story.points.to_string()))
        .fill(
            span().class("title").then(|title| if !story.url.starts_with("item?id=") {
                title.fill(
                    span().fill(a().href(story.url.clone()).target("_blank").rel("noreferrer").text(story.title.clone()))
                    .fill(span().class("host").html(format!("({})", story.domain.unwrap_or_default())))
                ) else {
                    title.fill(
                        a().href(format!("/stories/{}", story.id)).text(story.title.clone())
                    )
                }
            }))
            .fill(br())
            .fill(
                span().class("meta").then(|meta| {
                    if story.story_type != "job" {
                        meta.fill(
                            span().fill("by ").fill(
                                story.user.map(|user| a().href(format!("/users/{}", user)).text(user))
                            ).fill(format!(" {} | ", story.time_ago))
                            .fill(a().href(format!("/stories/{}", story.id)).text(
                                if story.comments_count.unwrap_or_default() > 0 {
                                    format!("{} comments", story.comments_count.unwrap_or_default())
                                } else {
                                    "discuss"
                                }
                            ))
                        )
                    } else {
                        meta.fill(a().href(format!("/item/{}", story.id)).text(&story.title))
                    }
            })).then(|meta|
                if story.story_type != "link" {
                meta.fill(" ").fill(span().class("label").html(&story.story_type))
            } else {meta}
        ).show_in(ctx);
    }
}

fn pluralize(n: usize) -> &'static str {
    if n == 1 {
        " reply"
    } else {
        " replies"
    }
}
