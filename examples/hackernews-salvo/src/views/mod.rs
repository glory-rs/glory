mod story;
mod user;

use std::rc::Rc;
use std::cell::RefCell;

use glory::reflow::*;
use glory::routing::*;
use glory::web::widgets::*;
use glory::widgets::*;
use glory::*;

use story::{ListStories, ShowStory};
use user::ShowUser;
use crate::models::*;

#[derive(Debug)]
pub struct App;
impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        glory::info!("App::build");
        let info = PageInfo::default();
        ctx.truck_mut().inject(info.clone());

        head_mixin()
            .fill(link().rel("stylesheet").href("/pkg/hackernews-salvo.css"))
            .fill(meta().name("description").content(info.description.clone()))
            .fill(title().html(info.title.clone()))
            .show_in(ctx);
        Graff::new("section").show_in(ctx);
    }
}

#[derive(Debug, Clone)]
struct NoMatch;
impl Widget for NoMatch {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "Not found page".to_owned());
        info.description.revise(|mut v| *v = "This is not found page".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        glory::info!("NoMatch::build");
        div()
            .fill(h2().html("Nothing to see here!"))
            .fill(a().attr("href", "/").html("Go to the home page"))
            .show_in(ctx);
    }
}

#[derive(Debug)]
pub struct Nav;
impl Widget for Nav {
    fn build(&mut self, ctx: &mut Scope) {
        header()
            .class("header")
            .fill(
                nav()
                    .class("inner")
                    .fill(a().href("/home").fill(strong().html("Home")))
                    .fill(a().href("/new").fill(strong().html("New")))
                    .fill(a().href("/show").fill(strong().html("Show")))
                    .fill(a().href("/ask").fill(strong().html("Ask")))
                    .fill(a().href("/job").fill(strong().html("Jobs")))
                    .fill(
                        a().class("github")
                            .href("http://github.com/glory-rs/glory")
                            .target("_blank")
                            .rel("noreferrer")
                            .html("Built with Glory"),
                    ),
            )
            .show_in(ctx);
    }
}

pub fn route() -> Router {
    Router::new()
        .push(Router::with_path("users/<id>").goal(|tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", ShowUser)))
        .push(Router::with_path("stories/<id>").goal(|tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", ShowStory)))
        .push(Router::with_path("<**stories>").goal(|tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", ListStories::new())))
}

pub fn catch() -> impl Handler {
    |tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", NoMatch)
}
