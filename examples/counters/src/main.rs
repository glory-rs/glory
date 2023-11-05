use std::sync::atomic::{AtomicUsize, Ordering};

use glory::reflow::Lotus;
use glory::web::events;
use glory::web::helpers::event_target_value;
use glory::web::widgets::*;
use glory::web::holders::BrowerHolder;
use glory::widgets::*;
use glory::*;

const MANY_COUNTERS: usize = 1000;
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

pub fn next_id() -> usize {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn main() {
    BrowerHolder::new().mount(Counters::new());
}
#[derive(Debug)]
struct Counters {
    counters: Cage<Vec<Counter>>,
}
impl Counters {
    pub fn new() -> Self {
        Self { counters: Cage::new(vec![]) }
    }
}

impl Widget for Counters {
    fn build(&mut self, ctx: &mut Scope) {
        let counters = self.counters.clone();
        let add_counter = move |_| {
            counters.revise(|mut counters| counters.push(Counter::new(next_id())));
        };

        let counters = self.counters.clone();
        let add_many_counters = move |_| {
            counters.revise(|mut counters| {
                let new_counters = (0..MANY_COUNTERS).map(|_| Counter::new(next_id()));
                (*counters).extend(new_counters);
            });
        };

        let counters = self.counters.clone();
        let clear_counters = move |_| {
            counters.revise(|mut counters| counters.clear());
        };
        button().on(events::click, add_counter).text("Add Counter").show_in(ctx);
        button()
            .on(events::click, add_many_counters)
            .text(format!("Add {MANY_COUNTERS} Counters"))
            .show_in(ctx);
        button().on(events::click, clear_counters).text("Clear Counters").show_in(ctx);

        let counters = self.counters.clone();
        let total = counters.map(|counters| counters.iter().map(|c| *c.value.get()).sum::<i32>().to_string());
        let count = counters.map(|counters| counters.len().to_string());
        p().fill(span().text("Total: "))
            .fill(span().text(total))
            .fill(span().text(" from "))
            .fill(span().text(count))
            .show_in(ctx);

        ul().fill(Each::new(
            counters,
            |counter| counter.id,
            |counter| {
                let counter = counter.to_owned();
                li().fill(
                    counter)
            },
        ))
        .show_in(ctx);
    }
}

#[derive(Debug, Clone)]
struct Counter {
    id: usize,
    value: Cage<i32>,
}
impl Counter {
    pub fn new(id: usize) -> Self {
        Self { id, value: Cage::new(0) }
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
