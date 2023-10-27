use std::cell::RefCell;
use std::rc::Rc;

use glory::reflow::*;
use glory::routing::*;
use glory::web::widgets::*;
use glory::widgets::*;
use glory::*;

#[derive(Debug, Clone)]
struct ShowUser;
impl Widget for ShowUser {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "User".to_owned());
        info.description.revise(|mut v| *v = "This is show user page".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("ShowUser::build");
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
            move |user, ctx| {
                if let Some(user) = user {
                    info.title.revise(|mut v| *v = format!("User:{}", user.id));
                    info.description.revise(|mut v| *v = user.about.clone());
                    div()
                        .fill(h1().html(format!("User:{}", user.id)))
                        .fill(
                            ul().class("meta")
                                .fill(li().fill(span().class("label").html("Created: ").fill(user.created)))
                                .fill(li().fill(span().class("label").html("Karma: ").fill(user.karma)))
                                .fill(user.about.map(|about| ul.fill(li().fill(span().class("about").html(about)))))
                                .fill(
                                    p().class("links")
                                        .fill(
                                            a().href(format!("https://news.ycombinator.com/submitted?id={}", user.id))
                                                .text("submissions"),
                                        )
                                        .fill(" | ")
                                        .fill(a().href(format!("https://news.ycombinator.com/threads?id={}", user.id)).text("comments")),
                                ),
                        )
                        .show_in(ctx);
                } else {
                    div().fill(h2().html("Not found")).show_in(ctx);
                }
            },
        )
        .fallback(|ctx| {
            p().html("Loading...").show_in(ctx);
        });

        div().class("user-view").fill(loader).show_in(ctx);
    }
}
