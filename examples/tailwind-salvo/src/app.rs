use std::cell::RefCell;
use std::rc::Rc;

use glory::reflow::*;
use glory::routing::*;
use glory::web::widgets::*;
use glory::*;

#[derive(Clone, Debug, Default)]
struct PageInfo {
    title: Cage<String>,
    description: Cage<String>,
    body_class: Cage<String>,
}

#[derive(Debug)]
pub struct App {}
impl App {
    pub fn new() -> Self {
        Self {}
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        info!("App::build");
        let info = PageInfo::default();
        ctx.truck_mut().inject(info.clone());
        head_mixin()
            .fill(link().rel("stylesheet").href("pkg/tailwind-salvo.css"))
            .fill(title().text("Glory + Tailwind"))
            .show_in(ctx);
        let path = ctx.truck().obtain::<Locator>().unwrap().path();
        div()
            .class("font-mono flex flex-col min-h-screen")
            .fill(
                ul().class("flex justify-between bg-gray-200")
                    .fill(
                        li().class("p-4")
                            .toggle_class("bg-blue-500 text-white", path.map(|path|path == "/"))
                            .fill(a().href("/").html("Home")),
                    )
                    .fill(
                        li().class("p-4")
                            .toggle_class("bg-blue-500 text-white", path.map(|path|path.starts_with("/dashboard")))
                            .fill(a().href("/dashboard").html("Dashboard")),
                    )
                    .fill(
                        li().class("p-4")
                            .toggle_class("bg-blue-500 text-white", path.map(|path|path.starts_with("/about")))
                            .fill(a().href("/about").html("About")),
                    ),
            )
            .fill(p().html("This example demonstrates a basic router that uses the browser's history API."))
            .fill(Graff::new("section"))
            .show_in(ctx);
    }
}

#[derive(Debug, Clone)]
struct Home;
impl Widget for Home {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "Home page".to_owned());
        info.description.revise(|mut v| *v = "This is home page".to_owned());
        info.body_class.revise(|mut v| *v = "home".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        div().fill(h2().html("Home")).show_in(ctx);
    }
}

#[derive(Debug, Clone)]
struct About;
impl Widget for About {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "About page".to_owned());
        info.description.revise(|mut v| *v = "This is about page".to_owned());
        info.body_class.revise(|mut v| *v = "about".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("About::build");
        div().fill(h2().html("About")).show_in(ctx);
    }
}

#[derive(Debug, Clone)]
struct Dashboard;
impl Widget for Dashboard {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "Dashboard page".to_owned());
        info.description.revise(|mut v| *v = "This is dashboard page".to_owned());
        info.body_class.revise(|mut v| *v = "dashboard".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("Dashboard::build");
        div().fill(h2().html("Dashboard")).show_in(ctx);
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
        info.body_class.revise(|mut v| *v = "not-found".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("NoMatch::build");
        div()
            .fill(h2().html("Nothing to see here!"))
            .fill(a().href("/").html("Go to the home page"))
            .show_in(ctx);
    }
}

pub fn route() -> Router {
    Router::new()
        .push(Router::with_path("dashboard").goal(|tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", Dashboard)))
        .push(Router::with_path("about").goal(|tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", About)))
        .goal(|tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", Home))
}

pub fn catch() -> impl Handler {
    |tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", NoMatch)
}
