//! Real desktop counter: the same widget code that runs on the web,
//! rendered in a native window through the command stream.
//!
//! ```text
//! cargo run -p desktop-counter
//! ```

use glory_core::reflow::Cage;
use glory_core::web::events;
use glory_core::web::helpers::event_target_value;
use glory_core::web::widgets::*;
use glory_core::{Scope, Widget};
use glory_desktop::{DesktopConfig, DesktopWindowHandle};

#[derive(Debug)]
struct Counter {
    value: Cage<i64>,
    window: DesktopWindowHandle,
}

impl Counter {
    fn new(window: DesktopWindowHandle) -> Self {
        Self {
            value: Cage::new(0),
            window,
        }
    }
}

impl Widget for Counter {
    fn build(&mut self, ctx: &mut Scope) {
        let value = self.value.clone();
        let increase = move |_| {
            value.revise(|mut value| *value += 1);
        };
        let value = self.value.clone();
        let decrease = move |_| {
            value.revise(|mut value| *value -= 1);
        };
        let value = self.value.clone();
        let clear = move |_| {
            value.revise(|mut value| *value = 0);
        };
        let value = self.value.clone();
        let set_from_input = move |ev| {
            let parsed = event_target_value(&ev).parse::<i64>().unwrap_or_default();
            value.revise(|mut value| *value = parsed);
        };
        let drag_window = {
            let window = self.window.clone();
            move |_| {
                window.drag_window();
            }
        };
        let toggle_fullscreen = {
            let window = self.window.clone();
            move |_| {
                window.set_fullscreen(!window.is_fullscreen());
            }
        };
        let toggle_maximized = {
            let window = self.window.clone();
            move |_| {
                window.toggle_maximized();
            }
        };
        let zoom_in = {
            let window = self.window.clone();
            move |_| {
                window.set_zoom_level((window.zoom_level() + 0.1).min(2.0));
            }
        };
        let zoom_reset = {
            let window = self.window.clone();
            move |_| {
                window.set_zoom_level(1.0);
            }
        };
        let open_window = {
            let window = self.window.clone();
            move |_| {
                window.open_window(
                    DesktopConfig {
                        title: "Runtime Window".to_owned(),
                        inner_size: (480.0, 200.0),
                        ..Default::default()
                    },
                    Counter::new,
                );
            }
        };
        let close_window = {
            let window = self.window.clone();
            move |_| {
                window.close();
            }
        };

        div()
            .attr(
                "style",
                "font-family: sans-serif; padding: 2em; display: flex; flex-wrap: wrap; gap: .5em; align-items: center;",
            )
            .fill(button().text("-").on(events::click, decrease))
            .fill(
                span()
                    .attr("style", "min-width: 4em; text-align: center; font-size: 1.5em;")
                    .text(self.value.clone()),
            )
            .fill(button().text("+").on(events::click, increase))
            .fill(button().text("clear").on(events::click, clear))
            .fill(input().attr("placeholder", "set value").on(events::input, set_from_input))
            .fill(button().text("drag").on(events::mousedown, drag_window))
            .fill(button().text("fullscreen").on(events::click, toggle_fullscreen))
            .fill(button().text("maximize").on(events::click, toggle_maximized))
            .fill(button().text("zoom +").on(events::click, zoom_in))
            .fill(button().text("zoom 1x").on(events::click, zoom_reset))
            .fill(button().text("new window").on(events::click, open_window))
            .fill(button().text("close").on(events::click, close_window))
            .show_in(ctx);
    }
}

fn main() {
    let config = DesktopConfig {
        title: "Glory Desktop Counter".to_owned(),
        inner_size: (480.0, 200.0),
        ..Default::default()
    };

    // `GLORY_MULTIWIN=1 cargo run` opens a second, independent window with
    // a native menu whose "+10" item drives that window's own counter.
    if std::env::var("GLORY_MULTIWIN").is_ok() {
        use glory_desktop::{Desktop, MenuItemSpec, MenuSpec};

        let second_value = glory_core::reflow::Cage::new(0i64);
        let menu_value = second_value.clone();
        let second = DesktopConfig {
            title: "Second Window".to_owned(),
            inner_size: (480.0, 200.0),
            menu: Some(MenuSpec::new().submenu("Counter", vec![MenuItemSpec::new("add-ten", "+10"), MenuItemSpec::new("reset", "Reset")])),
            on_menu: Some(std::rc::Rc::new(move |_holder, id| match id {
                "add-ten" => menu_value.revise(|mut v| *v += 10),
                "reset" => menu_value.revise(|mut v| *v = 0),
                _ => {}
            })),
            ..Default::default()
        };
        Desktop::new()
            .window_with_handle(config, Counter::new)
            .window_with_handle(second, move |window| Counter {
                value: second_value,
                window,
            })
            .run();
    }

    glory_desktop::launch_with_handle(config, Counter::new);
}
