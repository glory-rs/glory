use crate::model::Model;
use glory::display::*;
use std::cell::RefCell;
#[cfg(feature = "handwritten")]
#[path = "browser/handwritten.rs"]
mod handwritten;
#[cfg(not(feature = "handwritten"))]
#[path = "browser/xml.rs"]
mod xml;
struct Demo {
    app: App,
    model: Model,
}
thread_local! {static APP:RefCell<Option<Demo>>=const{RefCell::new(None)};}
pub fn main() {
    let app = App::at_id("main").unwrap();
    let model = Model::new(&app.scope());
    #[cfg(feature = "handwritten")]
    app.render(handwritten::view(model.clone())).unwrap();
    #[cfg(not(feature = "handwritten"))]
    app.render(xml::view(model.clone())).unwrap();
    APP.with(|s| *s.borrow_mut() = Some(Demo { app, model }));
}
#[path = "browser/statistics.rs"]
mod statistics;
