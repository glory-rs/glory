use core::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use glory::reflow::*;
use glory::routing::*;
use glory::web::widgets::*;
use glory::web::{event_target_checked, event_target_value, request_animation_frame, window_event_listener};
use glory::web::{events, NodeRef};
use glory::widgets::*;
use glory::*;
use web_sys::HtmlInputElement;

#[derive(Debug)]
pub struct App {}
impl App {
    pub fn new() -> Self {
        Self {}
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        div()
            .fill(h1().html("Basic Router Example"))
            .fill(
                ul().fill(li().fill(a().attr("href", "/").html("Home")))
                    .fill(li().fill(a().attr("href", "/dashboard").html("Dashboard")))
                    .fill(li().fill(a().attr("href", "/about").html("About"))),
            )
            .fill(p().html("This example demonstrates a basic router that uses the browser's history API."))
            .fill(div().fill(Graff::new("section")))
            .show_in(ctx);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Home;

impl Widget for Home {
    fn build(&mut self, ctx: &mut Scope) {
        div().fill(h2().html("Home")).show_in(ctx);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct About;

impl Widget for About {
    fn build(&mut self, ctx: &mut Scope) {
        div().fill(h2().html("About")).show_in(ctx);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Dashboard;

impl Widget for Dashboard {
    fn build(&mut self, ctx: &mut Scope) {
        div().fill(h2().html("Dashboard")).show_in(ctx);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NoMatch;

impl Widget for NoMatch {
    fn build(&mut self, ctx: &mut Scope) {
        div()
            .fill(h2().html("Nothing to see here!"))
            .fill(a().attr("href", "/").html("Go to the home page"))
            .show_in(ctx);
    }
}

pub fn route() -> Router {
    Router::new()
        .push(Router::with_path("dashboard").goal(|depot: Rc<RefCell<Depot>>| depot.insert_stuff("section", Dashboard)))
        .push(Router::with_path("about").goal(|depot: Rc<RefCell<Depot>>| depot.insert_stuff("section", About)))
        .push(Router::with_path("/").goal(|depot: Rc<RefCell<Depot>>| depot.insert_stuff("section", Home)))
}

pub fn catch() -> impl Handler {
    |depot: Rc<RefCell<Depot>>| depot.insert_stuff("section", NoMatch)
}
