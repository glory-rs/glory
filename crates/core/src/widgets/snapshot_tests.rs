//! SSR-driven snapshot tests for the keyed list / branch / loader widgets.
//!
//! These tests exercise the actual mount → patch → DOM-tree pipeline against
//! the in-memory `Node` backend, so they catch regressions in both the
//! widget logic and the surrounding scheduler/scope plumbing.

use std::cell::Cell;
use std::rc::Rc;

use crate::config::GloryConfig;
use crate::reflow::{Cage, effect_in, resource_in};
use crate::web::holders::ServerHolder;
use crate::web::widgets::{div, li, ul};
use crate::widgets::{Each, Switch};
use crate::{Holder, Scope, Widget};

fn render_html(holder: &ServerHolder) -> String {
    holder.host_node.node().inner_html()
}

fn make_holder() -> ServerHolder {
    ServerHolder::new(GloryConfig::default(), "/")
}

// ----------------------------------------------------------------------------
// Each
// ----------------------------------------------------------------------------

#[derive(Debug)]
struct EachListWidget {
    items: Cage<Vec<String>>,
}

impl Widget for EachListWidget {
    fn build(&mut self, ctx: &mut Scope) {
        ul().fill(Each::from_vec(self.items.clone(), |s| s.clone(), |s| li().text(s.clone())))
            .show_in(ctx);
    }
}

fn each_html_items(holder: &ServerHolder) -> Vec<String> {
    let html = render_html(holder);
    // Extract <li ...>VALUE</li> in order; values have no HTML special chars.
    let mut out = Vec::new();
    let mut rest = html.as_str();
    while let Some(open) = rest.find("<li") {
        rest = &rest[open..];
        let Some(close_open) = rest.find('>') else { break };
        rest = &rest[close_open + 1..];
        let Some(close_tag) = rest.find("</li>") else { break };
        out.push(rest[..close_tag].to_string());
        rest = &rest[close_tag + "</li>".len()..];
    }
    out
}

#[test]
fn each_initial_render() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c"]);
}

#[test]
fn each_append_tail() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| v.push("c".to_string()));
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c"]);

    items.revise(|mut v| v.push("d".to_string()));
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c", "d"]);
}

#[test]
fn each_prepend_head() {
    let items = Cage::new(vec!["b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| v.insert(0, "a".to_string()));
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c"]);
}

#[test]
fn each_reverse() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| v.reverse());
    assert_eq!(each_html_items(&holder), vec!["c", "b", "a"]);
}

#[test]
fn each_swap_adjacent() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| v.swap(1, 2));
    assert_eq!(each_html_items(&holder), vec!["a", "c", "b", "d"]);
}

#[test]
fn each_remove_middle() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| {
        v.remove(1);
    });
    assert_eq!(each_html_items(&holder), vec!["a", "c"]);
}

#[test]
fn each_clear() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| v.clear());
    assert!(each_html_items(&holder).is_empty());
}

#[test]
fn each_remove_then_readd_same_key() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| {
        v.remove(1);
    });
    items.revise(|mut v| v.insert(1, "b".to_string()));
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c"]);
}

#[test]
fn each_full_replacement_distinct_keys() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| {
        *v = vec!["x".to_string(), "y".to_string(), "z".to_string()];
    });
    assert_eq!(each_html_items(&holder), vec!["x", "y", "z"]);
}

#[test]
fn each_shuffle_keeps_all_keys() {
    let items = Cage::new(vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
    ]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| {
        *v = vec![
            "c".to_string(),
            "e".to_string(),
            "a".to_string(),
            "d".to_string(),
            "b".to_string(),
        ];
    });
    assert_eq!(each_html_items(&holder), vec!["c", "e", "a", "d", "b"]);
}

#[test]
fn each_large_reverse() {
    // Stress the LIS path on a non-trivial input so accidental
    // quadratic regressions show up.
    let n: usize = 200;
    let initial: Vec<String> = (0..n).map(|i| format!("k{i}")).collect();
    let items = Cage::new(initial.clone());
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| v.reverse());

    let expected: Vec<String> = initial.iter().rev().cloned().collect();
    assert_eq!(each_html_items(&holder), expected);
}

#[test]
fn each_large_random_shuffle() {
    // Deterministic pseudo-shuffle so the test is reproducible.
    let n: usize = 100;
    let initial: Vec<String> = (0..n).map(|i| format!("k{i}")).collect();
    let items = Cage::new(initial.clone());
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    // Cycle by step 7 (coprime to 100): produces an even spread.
    let shuffled: Vec<String> = (0..n).map(|i| initial[(i * 7) % n].clone()).collect();
    items.revise(|mut v| *v = shuffled.clone());

    assert_eq!(each_html_items(&holder), shuffled);
}

#[test]
fn each_property_random_reorders_match_target() {
    // Deterministic-random property test: from an initial set of 50
    // keys, apply 30 random permutations (using a simple LCG for
    // reproducibility) and assert each step's HTML matches the new
    // items. This is the in-process equivalent of `cargo-fuzz` for
    // the `Each::patch` invariant "after patch, DOM order == new
    // items order".
    let n: usize = 50;
    let initial: Vec<String> = (0..n).map(|i| format!("k{i}")).collect();
    let items = Cage::new(initial.clone());
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    // LCG: x_{n+1} = (a * x_n + c) mod m with values from Numerical Recipes
    let mut seed: u64 = 0xdead_beef_cafe;
    let mut next_u64 = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed
    };

    let mut current = initial.clone();
    for _ in 0..30 {
        // Random Fisher–Yates shuffle of a clone, sometimes drop one or
        // add a new key, sometimes reverse a slice.
        let op = next_u64() % 4;
        match op {
            0 => {
                // shuffle in place
                for i in (1..current.len()).rev() {
                    let j = (next_u64() as usize) % (i + 1);
                    current.swap(i, j);
                }
            }
            1 if !current.is_empty() => {
                // drop a random element
                let i = (next_u64() as usize) % current.len();
                current.remove(i);
            }
            2 => {
                // append a new unique key
                current.push(format!("new{}", next_u64() % 1_000_000));
            }
            _ => {
                // reverse a random slice
                if current.len() >= 2 {
                    let a = (next_u64() as usize) % current.len();
                    let b = (next_u64() as usize) % current.len();
                    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                    current[lo..=hi].reverse();
                }
            }
        }

        let expected = current.clone();
        items.revise(|mut v| *v = expected.clone());
        assert_eq!(
            each_html_items(&holder),
            expected,
            "DOM order diverged from target after a random reorder step"
        );
    }
}

#[test]
fn each_repeated_revisions_stay_consistent() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items: items.clone() });

    items.revise(|mut v| v.push("d".to_string()));
    items.revise(|mut v| v.reverse());
    items.revise(|mut v| {
        v.remove(0);
    });
    items.revise(|mut v| v.insert(0, "z".to_string()));

    // After: push d → [a,b,c,d]; reverse → [d,c,b,a]; remove 0 → [c,b,a];
    //        insert z at 0 → [z,c,b,a].
    assert_eq!(each_html_items(&holder), vec!["z", "c", "b", "a"]);
}

// ----------------------------------------------------------------------------
// Switch (with cached_view)
// ----------------------------------------------------------------------------

#[derive(Debug)]
struct SwitchHostWidget {
    show_left: Cage<bool>,
}

impl Widget for SwitchHostWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let show_left = self.show_left.clone();
        let show_right = self.show_left.map(|b| !*b);
        div()
            .fill(
                Switch::new()
                    .push(
                        crate::widgets::switch::Case::new(show_left, || {
                            div().class("left").text("LEFT")
                        })
                        .cache(true),
                    )
                    .push(crate::widgets::switch::Case::new(show_right, || {
                        div().class("right").text("RIGHT")
                    })),
            )
            .show_in(ctx);
    }
}

// ----------------------------------------------------------------------------
// Effect
// ----------------------------------------------------------------------------

#[derive(Debug)]
struct EffectHostWidget {
    count: Cage<i32>,
    runs: Rc<Cell<i32>>,
    seen: Rc<Cell<i32>>,
}

impl Widget for EffectHostWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let count = self.count.clone();
        let runs = self.runs.clone();
        let seen = self.seen.clone();
        effect_in(ctx, move || {
            runs.set(runs.get() + 1);
            seen.set(*count.get());
        });
        // Render something so the SSR backend doesn't trip over an empty
        // root scope.
        div().show_in(ctx);
    }
}

// ----------------------------------------------------------------------------
// Resource (async derived signal)
// ----------------------------------------------------------------------------

#[derive(Debug)]
struct ResourceHostWidget {
    seed: Cage<i32>,
    result_handle: Rc<std::cell::RefCell<Option<Cage<Option<i32>>>>>,
}

impl Widget for ResourceHostWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let seed = self.seed.clone();
        let cell = resource_in(ctx, move || {
            let v = *seed.get();
            async move { v * 10 }
        });
        *self.result_handle.borrow_mut() = Some(cell);
        div().show_in(ctx);
    }
}

#[test]
fn resource_resolves_initial_and_after_dep_change() {
    let seed = Cage::new(2_i32);
    let handle: Rc<std::cell::RefCell<Option<Cage<Option<i32>>>>> = Rc::new(std::cell::RefCell::new(None));
    let _holder = make_holder().mount(ResourceHostWidget {
        seed: seed.clone(),
        result_handle: handle.clone(),
    });

    let cell = handle.borrow().clone().expect("resource cell was published");
    assert_eq!(*cell.get(), Some(20));

    seed.revise(|mut v| *v = 7);
    assert_eq!(*cell.get(), Some(70));
}

#[test]
fn effect_runs_once_on_mount_then_per_revision() {
    let count = Cage::new(0_i32);
    let runs = Rc::new(Cell::new(0));
    let seen = Rc::new(Cell::new(-1));
    let _holder = make_holder().mount(EffectHostWidget {
        count: count.clone(),
        runs: runs.clone(),
        seen: seen.clone(),
    });

    // Initial run during build.
    assert_eq!(runs.get(), 1, "effect should run once on mount");
    assert_eq!(seen.get(), 0);

    count.revise(|mut v| *v = 7);
    assert_eq!(runs.get(), 2);
    assert_eq!(seen.get(), 7);

    count.revise(|mut v| *v = 7);
    // Cage::revise bumps the version every write, so the effect re-runs
    // even though the value is unchanged (matches today's `Cage` semantics
    // — `Cage` does not deduplicate writes; use `Bond::with_partial_eq`
    // when that's the goal).
    assert_eq!(runs.get(), 3);
}

#[test]
fn switch_toggles_and_restores_cached_view() {
    let show_left = Cage::new(true);
    let holder = make_holder().mount(SwitchHostWidget { show_left: show_left.clone() });

    let initial = render_html(&holder);
    assert!(initial.contains("LEFT"));
    assert!(!initial.contains("RIGHT"));

    show_left.revise(|mut v| *v = false);
    let after_swap = render_html(&holder);
    assert!(!after_swap.contains("LEFT"));
    assert!(after_swap.contains("RIGHT"));

    // Flipping back should re-mount the cached LEFT view without panicking
    // and without dropping the RIGHT subtree leftover behind.
    show_left.revise(|mut v| *v = true);
    let after_restore = render_html(&holder);
    assert!(after_restore.contains("LEFT"));
    assert!(!after_restore.contains("RIGHT"));
}

