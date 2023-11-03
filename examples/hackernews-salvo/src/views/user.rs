use glory::reflow::*;
use glory::routing::*;
use glory::web::widgets::*;
use glory::widgets::*;
use glory::*;

use crate::models::*;

#[derive(Debug, Clone)]
pub struct ShowUser;
impl Widget for ShowUser {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "User".to_owned());
        info.description.revise(|mut v| *v = "This is show user page".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("ShowUser::build");
        let user_id = {
            let truck = ctx.truck();
            let locator = truck.obtain::<Locator>().unwrap();
            locator.params().get().get("id").cloned().unwrap_or_default()
        };
        let info = {
            let truck = ctx.truck();
            truck.obtain::<PageInfo>().unwrap().clone()
        };
        let loader = Loader::new(
            move || {
                let api_url = user_api_url(user_id.clone());
                async move { fetch_api::<User>(&api_url).await }
            },
            move |user, ctx| {
                if let Some(user) = user {
                    info.title.revise(|mut v| *v = format!("User:{}", user.id));
                    info.description.revise(|mut v| *v = user.about.clone().unwrap_or_default());

                    h1().html(format!("User:{}", user.id)).show_in(ctx);
                    ul().class("meta")
                        .fill(li().fill(span().class("label").html("Created: ").fill(user.created.to_string())))
                        .fill(li().fill(span().class("label").html("Karma: ").fill(user.karma.to_string())))
                        .fill(user.about.clone().map(|about| ul().fill(li().fill(span().class("about").html(about)))))
                        .fill(
                            p().class("links")
                                .fill(
                                    a().href(format!("https://news.ycombinator.com/submitted?id={}", user.id))
                                        .text("submissions"),
                                )
                                .fill(" | ")
                                .fill(a().href(format!("https://news.ycombinator.com/threads?id={}", user.id)).text("comments")),
                        )
                        .show_in(ctx);
                } else {
                    h2().html("User not found").show_in(ctx);
                }
            },
        )
        .fallback(|ctx| {
            div().class("loading").fill(p().html("Loading user...")).show_in(ctx);
        });

        div().class("user-view").fill(loader).show_in(ctx);
    }
}
