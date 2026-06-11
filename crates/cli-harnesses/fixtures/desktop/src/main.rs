use glory_core::web::widgets::*;
use glory_core::{Scope, Widget};
use glory_desktop::DesktopConfig;

#[derive(Debug)]
struct App;

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        div().text("desktop harness").show_in(ctx);
    }
}

fn main() {
    glory_desktop::launch_with_config(
        DesktopConfig {
            title: "Glory Harness".to_owned(),
            ..Default::default()
        },
        || App,
    );
}
