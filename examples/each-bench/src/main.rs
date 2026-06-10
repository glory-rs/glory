use std::hint::black_box;
use std::time::{Duration, Instant};

use glory_core::config::GloryConfig;
use glory_core::reflow::Cage;
use glory_core::web::holders::ServerHolder;
use glory_core::web::widgets::{li, ul};
use glory_core::widgets::Each;
use glory_core::{Holder, Scope, Widget};

const SIZES: [usize; 3] = [10, 100, 1000];

#[derive(Debug)]
struct ListWidget {
    items: Cage<Vec<String>>,
}

impl Widget for ListWidget {
    fn build(&mut self, ctx: &mut Scope) {
        ul().fill(Each::from_vec(self.items.clone(), |s| s.clone(), |s| li().text(s.clone())))
            .show_in(ctx);
    }
}

fn main() {
    println!("workload,n,iterations,total_us,avg_us");
    for n in SIZES {
        let shuffled = shuffled_items(n);
        run_workload("reverse", n, |items| {
            items.revise(|mut values| values.reverse());
        });
        run_workload("shuffle", n, {
            let shuffled = shuffled.clone();
            move |items| {
                let shuffled = shuffled.clone();
                items.revise(|mut values| *values = shuffled);
            }
        });
        run_workload("insert_head", n, |items| {
            items.revise(|mut values| values.insert(0, "new-head".to_string()));
        });
        run_workload("insert_tail", n, |items| {
            items.revise(|mut values| values.push("new-tail".to_string()));
        });
        run_workload("clear", n, |items| {
            items.revise(|mut values| values.clear());
        });
    }
}

fn run_workload<F>(name: &str, n: usize, mut mutate: F)
where
    F: FnMut(&Cage<Vec<String>>),
{
    let iterations = iterations_for(n);
    let mut total = Duration::ZERO;

    for _ in 0..iterations {
        let items = Cage::new(make_items(n));
        let holder = make_holder().mount(ListWidget { items: items.clone() });

        let started = Instant::now();
        mutate(&items);
        total += started.elapsed();

        black_box(holder.app_html());
    }

    let avg_us = total.as_secs_f64() * 1_000_000.0 / iterations as f64;
    println!("{name},{n},{iterations},{},{avg_us:.2}", total.as_micros());
}

fn iterations_for(n: usize) -> usize {
    match n {
        0..=10 => 200,
        11..=100 => 100,
        _ => 30,
    }
}

fn make_holder() -> ServerHolder {
    ServerHolder::new(GloryConfig::default(), "/")
}

fn make_items(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("k{i}")).collect()
}

fn shuffled_items(n: usize) -> Vec<String> {
    let mut values = make_items(n);
    let mut state = 0x9E37_79B9_7F4A_7C15_u64 ^ n as u64;

    for i in (1..values.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let j = (state as usize) % (i + 1);
        values.swap(i, j);
    }

    values
}
