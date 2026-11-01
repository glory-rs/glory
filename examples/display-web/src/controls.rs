//! Fixture-only event adapters over Glory's own semantic controls.
#![allow(dead_code, unused_imports)] // Each standalone fixture uses a different subset.
pub use g::ui::{Button, Input};

pub trait Press: Sized {
    fn on_press(self, callback: impl Fn() + 'static) -> Self;
}
impl Press for Button {
    fn on_press(self, callback: impl Fn() + 'static) -> Self {
        self.on(g::ui::events::activate, move |_| callback())
    }
}
pub trait ValueChange: Sized {
    fn on_value_change(self, callback: impl Fn(String) + 'static) -> Self;
}
impl ValueChange for Input {
    fn on_value_change(self, callback: impl Fn(String) + 'static) -> Self {
        self.on(g::ui::events::value_changed, move |event| {
            if let g::ui::Event::ValueChanged { value, .. } = event {
                callback(value);
            }
        })
    }
}
