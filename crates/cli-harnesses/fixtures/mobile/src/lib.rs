use glory_core::web::widgets::*;
use glory_core::{Scope, Widget};

#[derive(Debug)]
pub struct App;

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        div().text("mobile harness").show_in(ctx);
    }
}

#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn start_app() {}
