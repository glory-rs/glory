use glory::web::events;
use glory::web::helpers::event_target_value;
use glory::web::widgets::*;
use glory::web::holders::BrowerHolder;
use glory::*;

pub fn main() {
    BrowerHolder::new().mount(Counter::new());
}

#[derive(Debug)]
struct Counter {
    value: Cage<i32>,
}
impl Counter {
    pub fn new() -> Self {
        Self { value: Cage::new(0) }
    }
}

impl Widget for Counter {
    fn build(&mut self, ctx: &mut Scope) {
        let value = self.value.clone();
        let clear_counter = move |_| {
            value.revise(|mut value| {
                *value = 0;
            });
        };
        let value = self.value.clone();
        let decrease_counter = move |_| {
            value.revise(|mut value| {
                *value = *value - 1;
            });
        };
        let value = self.value.clone();
        let increase_counter = move |_| {
            value.revise(|mut value| {
                *value = *value + 1;
            });
        };

        let value = self.value.clone();
        let input_counter = move |ev| {
            let v = event_target_value(&ev).parse::<i32>().unwrap_or_default();
            value.revise(|mut value| {
                *value = v;
            });
        };

        let value = self.value.clone();
        let text = self.value.map(|value| format!("Value: {value}"));
        p().class("counter")
            .fill(button().on(events::click, clear_counter).html("Clear"))
            .fill(button().on(events::click, decrease_counter).html("-1"))
            .fill(input().on(events::input, input_counter).prop("value", value))
            .fill(span().html(text.clone()))
            .fill(button().on(events::click, increase_counter).html("+1"))
            .show_in(ctx);
    }
}
