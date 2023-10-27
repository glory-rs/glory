use std::cell::RefCell;
use std::rc::Rc;

use glory::reflow::*;
use glory::routing::*;
use glory::web::widgets::*;
use glory::widgets::*;
use glory::*;

pub struct ShowStory;
impl Widget for ShowStory {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "User".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("ShowStory::build");
        let user_id: usize = {
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
            || models::fetch_api::<User>(&api::user_api_url(&id)).await,
            move |story, ctx| {
                if let Some(story) = user {
                    info.title.revise(|mut v| *v = story.title);
                    info.description.revise(|mut v| *v = story.title.clone());
                    div()
                        .class("item-view-header")
                        .fill(a().href(&story.url).target("_blank").fill(h1().html(&story.title)))
                        .fill(story.user.map(|user| {
                            p().class("meta")
                                .fill(story.points.to_string())
                                .fill(" points | by ")
                                .fill(a().href(format!("/users/{user}")).html(&user))
                                .fill(format!(" {}", story.time_ago))
                        }))
                        .show_in(ctx);
                    div()
                        .class("item-view-comments")
                        .fill(
                            p().class("item-view-comments-header")
                                .fill(if story.comments_count.unwrap_or_default() > 0 {
                                    format!("{} comments", story.comments_count.unwrap_or_default())
                                } else {
                                    "No comments yet.".into()
                                })
                                .fill(ul().class("comment-children").fill(Each::new(
                                    story.comments.iter(),
                                    |comment| comment.id,
                                    |comment|ShowComment::new(comment.clone()) ,
                                ))),
                        )
                        .show_in(ctx);
                } else {
                    h2().html("Story not found").show_in(ctx);
                }
            },
        )
        .fallback(|ctx| {
            p().html("Loading story...").show_in(ctx);
        });

        div().class("user-view").fill(loader).show_in(ctx);
    }
}

pub struct ShowComment{
    comment: Comment,
    opened: Cage<bool>,
}
impl ShowComment {
    pub fn new(comment: Comment) -> Self {
        Self {
            comment,
            opened: Cage::new(false),
            children: Cage::new(vec![]),
        }
    }
}
impl Widget for ShowComment {
    fn build(&mut self, ctx: &mut Scope) {
        li().class("comment").fill(
            div().class("by").fill(
                a().href(format!("/users/{}", self.user.clone().unwrap_or_default())).text(self.user.clone())
            ).fill(format!(" {}", self.time_ago))
        ).fill(
            div().class("text").html(&self.content)
        ).then(|li| if !self.comments.is_empty() {
            li.fill(
                div().fill(
                    div().class("toggle").toggle_class("open", ||{
                        self.opened.get()
                    }).fill(
                        a().on(events::click, move|e|{
                            let opened = ! self.opened.get();
                            self.opened.revise(|v|*v = opened);
                            self.children.revise(|v|*v = if opened {
                                self.comment.comments.clone()
                            } else {
                                vec![]
                            });
                        }).html(Bond::new(||{
                            if self.opened.get() {
                                "[-]"
                            } else {
                                let len = self.comment.comments.len()
                                format!("[+] {}{} collapsed", len, pluralize(len))
                            }
                        }))
                    ).fill(
                        Switch::new().case(self.opened, |ctx|{
                            ul().class("comment-children").fill(Each::new(
                                self.comment.comments.get().iter(),
                                |comment| comment.id,
                                |comment| ShowComment::new(comment.clone()),
                            )).show_in(ctx);
                        })
                    )
                )
            )
        } else {
            li
        }).show_in(ctx);
    }
}

fn pluralize(n: usize) -> &'static str {
    if n == 1 {
        " reply"
    } else {
        " replies"
    }
}
