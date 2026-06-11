use glory::web::holders::BrowserHolder;
use glory::web::widgets::*;
use glory::{Scope, Widget};

#[derive(Debug)]
struct App;

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        div().text("web harness").show_in(ctx);
    }
}

fn main() {
    BrowserHolder::new().mount(App);
}
