//! SSR-driven snapshot tests for the keyed list / branch / loader widgets.
//!
//! These tests exercise the actual mount → patch → DOM-tree pipeline against
//! the in-memory `Node` backend, so they catch regressions in both the
//! widget logic and the surrounding scheduler/scope plumbing.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::config::GloryConfig;
use crate::reflow::{Cage, Revisable, effect_in, resource_in};
use crate::scope::SuspenseBoundary as SuspenseBoundaryHandle;
use crate::web::holders::ServerHolder;
use crate::web::widgets::{
    button, div, form, head_mixin, input, label, li, link, math as math_widgets, meta, option, select, style, svg as svg_widgets, textarea, title, ul,
};
use crate::widgets::{Each, ErrorBoundary, Suspense, Switch};
use crate::{Holder, Scope, Widget};

fn render_html(holder: &ServerHolder) -> String {
    holder.app_html()
}

fn make_holder() -> ServerHolder {
    ServerHolder::new(GloryConfig::default(), "/")
}

// ----------------------------------------------------------------------------
// Suspense
// ----------------------------------------------------------------------------

#[derive(Debug)]
struct ManualSuspendWidget {
    boundary: Rc<RefCell<Option<SuspenseBoundaryHandle>>>,
}

impl Widget for ManualSuspendWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let boundary = ctx.suspense_boundary.expect("ManualSuspendWidget must be under Suspense");
        boundary.start();
        *self.boundary.borrow_mut() = Some(boundary);
        div().class("body").text("ready").show_in(ctx);
    }
}

#[test]
fn suspense_shows_fallback_until_pending_boundary_resolves() {
    let boundary = Rc::new(RefCell::new(None));
    let holder = make_holder().mount(Suspense::new(ManualSuspendWidget { boundary: boundary.clone() }, |ctx| {
        div().class("fallback").text("loading").show_in(ctx);
    }));

    let html = render_html(&holder);
    assert!(html.contains("loading"), "{html}");
    assert!(!html.contains("ready"), "{html}");

    boundary.borrow().expect("suspense boundary captured").finish();

    let html = render_html(&holder);
    assert!(html.contains("ready"), "{html}");
    assert!(!html.contains("loading"), "{html}");
}

// ----------------------------------------------------------------------------
// Error boundaries
// ----------------------------------------------------------------------------

#[derive(Debug)]
struct PanicBuildWidget;

impl Widget for PanicBuildWidget {
    fn build(&mut self, _ctx: &mut Scope) {
        panic!("build failed");
    }
}

#[test]
fn error_boundary_renders_fallback_when_child_build_panics() {
    let holder = make_holder().mount(ErrorBoundary::new(PanicBuildWidget, |error, ctx| {
        div().class("error").text(error.message().to_owned()).show_in(ctx);
    }));

    let html = render_html(&holder);
    assert!(html.contains(r#"class="error""#), "{html}");
    assert!(html.contains("build failed"), "{html}");
    let host_html = holder.replay().outer_html(holder.host_node.node().id());
    assert!(host_html.contains("gly-error-0"), "{host_html}");
}

#[derive(Debug)]
struct PanicPatchWidget {
    trigger: Cage<bool>,
}

impl Widget for PanicPatchWidget {
    fn build(&mut self, ctx: &mut Scope) {
        self.trigger.bind_view(ctx.view_id());
        div().text("ready").show_in(ctx);
    }

    fn patch(&mut self, _ctx: &mut Scope) {
        panic!("patch failed");
    }
}

#[derive(Debug)]
struct PatchBoundaryHost {
    trigger: Cage<bool>,
}

impl Widget for PatchBoundaryHost {
    fn build(&mut self, ctx: &mut Scope) {
        ErrorBoundary::new(PanicPatchWidget { trigger: self.trigger }, |error, ctx| {
            div()
                .class("error")
                .attr("data-source", error.source().unwrap_or("").to_owned())
                .text(error.message().to_owned())
                .show_in(ctx);
        })
        .show_in(ctx);
    }
}

#[test]
fn error_boundary_catches_child_patch_panics() {
    let trigger = Cage::new(false);
    let holder = make_holder().mount(PatchBoundaryHost { trigger });
    assert!(render_html(&holder).contains("ready"));

    trigger.revise(|mut value| *value = true);

    let html = render_html(&holder);
    assert!(html.contains(r#"class="error""#), "{html}");
    assert!(html.contains("patch failed"), "{html}");
    assert!(html.contains(r#"data-source="0-0-0""#), "{html}");
    assert!(!html.contains("ready"), "{html}");
}

// ----------------------------------------------------------------------------
// Document head
// ----------------------------------------------------------------------------

#[derive(Debug)]
struct HeadDocumentWidget;

impl Widget for HeadDocumentWidget {
    fn build(&mut self, ctx: &mut Scope) {
        head_mixin()
            .fill(title().text("Glory App"))
            .fill(meta().attr("name", "description").attr("content", "Builder UI"))
            .fill(link().attr("rel", "canonical").attr("href", "https://glory.rs/"))
            .show_in(ctx);
        div().text("body").show_in(ctx);
    }
}

#[test]
fn document_head_mixin_renders_into_ssr_head() {
    let holder = make_holder().mount(HeadDocumentWidget);
    let html = holder.render_string();
    assert!(html.contains("<title"), "{html}");
    assert!(html.contains("Glory App"), "{html}");
    assert!(html.contains(r#"name="description""#), "{html}");
    assert!(html.contains(r#"content="Builder UI""#), "{html}");
    assert!(html.contains(r#"rel="canonical""#), "{html}");
    assert!(html.contains(r#"href="https://glory.rs/""#), "{html}");
}

#[derive(Debug)]
struct ScopedStyleWidget;

impl Widget for ScopedStyleWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let scope = crate::web::scoped_css(":scope > button { color: red; }");
        style().text(scope.css().to_owned()).show_in(ctx);
        div().class(scope.clone()).fill(button().text("Save")).show_in(ctx);
    }
}

#[test]
fn scoped_style_renders_style_and_scope_class() {
    let holder = make_holder().mount(ScopedStyleWidget);
    let html = render_html(&holder);
    let scope = crate::web::scoped_css(":scope > button { color: red; }");
    assert!(html.contains(scope.class_name()), "{html}");
    assert!(html.contains(&format!(".{} &gt; button", scope.class_name())), "{html}");
    assert!(html.contains(r#"<button gly-id="#), "{html}");
}

// ----------------------------------------------------------------------------
// Markup surface
// ----------------------------------------------------------------------------

#[derive(Debug)]
struct FormSurfaceWidget;

impl Widget for FormSurfaceWidget {
    fn build(&mut self, ctx: &mut Scope) {
        form()
            .attr("method", "post")
            .attr("action", "/todos")
            .fill(label().attr("for", "todo-title").text("Title"))
            .fill(
                input()
                    .id("todo-title")
                    .attr("name", "title")
                    .attr("value", "Buy milk")
                    .attr("required", true),
            )
            .fill(input().attr("type", "checkbox").attr("name", "done").attr("checked", true))
            .fill(
                select()
                    .attr("name", "priority")
                    .fill(option().attr("value", "high").attr("selected", true).text("High")),
            )
            .fill(textarea().attr("name", "notes").text("Bring bags"))
            .fill(button().attr("type", "submit").text("Save"))
            .show_in(ctx);
    }
}

#[derive(Debug)]
struct SvgMathSurfaceWidget;

impl Widget for SvgMathSurfaceWidget {
    fn build(&mut self, ctx: &mut Scope) {
        div()
            .fill(
                svg_widgets::svg()
                    .attr("viewBox", "0 0 10 10")
                    .attr("role", "img")
                    .fill(
                        svg_widgets::defs()
                            .fill(
                                svg_widgets::linear_gradient()
                                    .attr("id", "grad")
                                    .fill(svg_widgets::stop().attr("offset", "0%")),
                            )
                            .fill(svg_widgets::filter().attr("id", "shadow").fill(svg_widgets::fe_drop_shadow()))
                            .fill(svg_widgets::clip_path().attr("id", "clip").fill(svg_widgets::rect())),
                    )
                    .fill(svg_widgets::title().text("Glory badge"))
                    .fill(
                        svg_widgets::circle()
                            .attr("cx", "5")
                            .attr("cy", "5")
                            .attr("r", "4")
                            .attr("fill", "currentColor"),
                    )
                    .fill(svg_widgets::path().attr("id", "curve").attr("d", "M1 8 C3 2 7 2 9 8"))
                    .fill(svg_widgets::text().fill(svg_widgets::text_path().attr("href", "#curve").text("G")))
                    .fill(svg_widgets::switch_().fill(svg_widgets::use_().attr("href", "#badge")))
                    .fill(svg_widgets::text().attr("x", "5").attr("y", "6").text("G")),
            )
            .fill(
                math_widgets::math().fill(
                    math_widgets::mrow()
                        .fill(math_widgets::mi().text("x"))
                        .fill(math_widgets::mo().text("="))
                        .fill(
                            math_widgets::mfrac()
                                .fill(math_widgets::mn().text("1"))
                                .fill(math_widgets::mn().text("2")),
                        ),
                ),
            )
            .fill(
                math_widgets::math()
                    .fill(
                        math_widgets::mroot()
                            .fill(math_widgets::mi().text("x"))
                            .fill(math_widgets::mn().text("3")),
                    )
                    .fill(math_widgets::merror().fill(math_widgets::mtext().text("bad")))
                    .fill(math_widgets::semantics().fill(math_widgets::annotation_xml().attr("encoding", "application/xhtml+xml"))),
            )
            .show_in(ctx);
    }
}

#[test]
fn form_controls_render_expected_ssr_markup() {
    let holder = make_holder().mount(FormSurfaceWidget);
    let html = render_html(&holder);
    assert!(html.contains("<form"), "{html}");
    assert!(html.contains(r#"method="post""#), "{html}");
    assert!(html.contains(r#"action="/todos""#), "{html}");
    assert!(html.contains(r#"name="title""#), "{html}");
    assert!(html.contains(r#"value="Buy milk""#), "{html}");
    assert!(html.contains(r#"type="checkbox""#), "{html}");
    assert!(html.contains(r#"<select"#), "{html}");
    assert!(html.contains(r#"<option"#), "{html}");
    assert!(html.contains("Bring bags"), "{html}");
    assert!(html.contains("Save"), "{html}");
}

#[test]
fn svg_and_mathml_render_expected_ssr_markup() {
    let holder = make_holder().mount(SvgMathSurfaceWidget);
    let html = render_html(&holder);
    assert!(html.contains("<svg"), "{html}");
    assert!(html.contains(r#"viewBox="0 0 10 10""#), "{html}");
    assert!(html.contains("<circle"), "{html}");
    assert!(html.contains("<linearGradient"), "{html}");
    assert!(html.contains("<feDropShadow"), "{html}");
    assert!(html.contains("<clipPath"), "{html}");
    assert!(html.contains("<textPath"), "{html}");
    assert!(html.contains("<switch"), "{html}");
    assert!(html.contains("<use"), "{html}");
    assert!(html.contains("Glory badge"), "{html}");
    assert!(html.contains("<math"), "{html}");
    assert!(html.contains("<mfrac"), "{html}");
    assert!(html.contains("<mroot"), "{html}");
    assert!(html.contains("<merror"), "{html}");
    assert!(html.contains("<annotation-xml"), "{html}");
    assert!(html.contains("<mn"), "{html}");
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
        ul().fill(Each::from_vec(self.items, |s| s.clone(), |s| li().text(s.clone())))
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
    let holder = make_holder().mount(EachListWidget { items });
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c"]);
}

#[test]
fn each_append_tail() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

    items.revise(|mut v| v.push("c".to_string()));
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c"]);

    items.revise(|mut v| v.push("d".to_string()));
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c", "d"]);
}

#[test]
fn each_prepend_head() {
    let items = Cage::new(vec!["b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

    items.revise(|mut v| v.insert(0, "a".to_string()));
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c"]);
}

#[test]
fn each_reverse() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

    items.revise(|mut v| v.reverse());
    assert_eq!(each_html_items(&holder), vec!["c", "b", "a"]);
}

#[test]
fn each_swap_adjacent() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

    items.revise(|mut v| v.swap(1, 2));
    assert_eq!(each_html_items(&holder), vec!["a", "c", "b", "d"]);
}

#[test]
fn each_remove_middle() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

    items.revise(|mut v| {
        v.remove(1);
    });
    assert_eq!(each_html_items(&holder), vec!["a", "c"]);
}

#[test]
fn each_clear() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

    items.revise(|mut v| v.clear());
    assert!(each_html_items(&holder).is_empty());
}

#[test]
fn each_remove_then_readd_same_key() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

    items.revise(|mut v| {
        v.remove(1);
    });
    items.revise(|mut v| v.insert(1, "b".to_string()));
    assert_eq!(each_html_items(&holder), vec!["a", "b", "c"]);
}

#[test]
fn each_full_replacement_distinct_keys() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

    items.revise(|mut v| {
        *v = vec!["x".to_string(), "y".to_string(), "z".to_string()];
    });
    assert_eq!(each_html_items(&holder), vec!["x", "y", "z"]);
}

#[test]
fn each_shuffle_keeps_all_keys() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string(), "e".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

    items.revise(|mut v| {
        *v = vec!["c".to_string(), "e".to_string(), "a".to_string(), "d".to_string(), "b".to_string()];
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
    let holder = make_holder().mount(EachListWidget { items });

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
    let holder = make_holder().mount(EachListWidget { items });

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
    let holder = make_holder().mount(EachListWidget { items });

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
fn each_on_enter_on_exit_hooks_fire() {
    #[derive(Debug)]
    struct EachWithHooks {
        items: Cage<Vec<String>>,
        enter_count: Rc<Cell<usize>>,
        exit_count: Rc<Cell<usize>>,
    }
    impl Widget for EachWithHooks {
        fn build(&mut self, ctx: &mut Scope) {
            let enter_count = self.enter_count.clone();
            let exit_count = self.exit_count.clone();
            ul().fill(
                Each::from_vec(self.items, |s| s.clone(), |s| li().text(s.clone()))
                    .on_enter(move |_vid| enter_count.set(enter_count.get() + 1))
                    .on_exit(move |_vid| exit_count.set(exit_count.get() + 1)),
            )
            .show_in(ctx);
        }
    }

    let enter_count = Rc::new(Cell::new(0_usize));
    let exit_count = Rc::new(Cell::new(0_usize));
    let items = Cage::new(vec!["a".to_string(), "b".to_string()]);
    let _holder = make_holder().mount(EachWithHooks {
        items,
        enter_count: enter_count.clone(),
        exit_count: exit_count.clone(),
    });

    // Initial render: two new rows → two enters, no exits.
    assert_eq!(enter_count.get(), 2, "initial render fires on_enter per row");
    assert_eq!(exit_count.get(), 0);

    items.revise(|mut v| v.push("c".to_string()));
    assert_eq!(enter_count.get(), 3);
    assert_eq!(exit_count.get(), 0);

    items.revise(|mut v| {
        v.remove(0);
    });
    assert_eq!(enter_count.get(), 3);
    assert_eq!(exit_count.get(), 1);

    items.revise(|mut v| v.reverse());
    assert_eq!(enter_count.get(), 3, "reorder does not fire on_enter");
    assert_eq!(exit_count.get(), 1, "reorder does not fire on_exit");
}

#[test]
fn each_supports_vec_deque() {
    use std::collections::VecDeque;

    #[derive(Debug)]
    struct VecDequeListWidget {
        items: Cage<VecDeque<String>>,
    }
    impl Widget for VecDequeListWidget {
        fn build(&mut self, ctx: &mut Scope) {
            // Turbofish needed because `Lotus<T>` also implements
            // `From<Lotus<T>>` into `Lotus<Option<T>>`, making
            // `impl Into<Lotus<_>>` ambiguous on bare collection
            // containers other than the `from_vec` shortcut.
            let each: Each<String, VecDeque<String>, _, String, _, crate::web::widgets::HtmlLi> =
                Each::new(self.items, |s: &String| s.clone(), |s: &String| li().text(s.clone()));
            ul().fill(each).show_in(ctx);
        }
    }

    let mut initial = VecDeque::new();
    initial.push_back("a".to_string());
    initial.push_back("b".to_string());
    initial.push_back("c".to_string());
    let items = Cage::new(initial);
    let holder = make_holder().mount(VecDequeListWidget { items });

    assert_eq!(each_html_items(&holder), vec!["a", "b", "c"]);

    // Push to the front (the move VecDeque is specifically good at).
    items.revise(|mut v| v.push_front("z".to_string()));
    assert_eq!(each_html_items(&holder), vec!["z", "a", "b", "c"]);

    items.revise(|mut v| {
        v.pop_back();
    });
    assert_eq!(each_html_items(&holder), vec!["z", "a", "b"]);
}

#[test]
fn each_repeated_revisions_stay_consistent() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let holder = make_holder().mount(EachListWidget { items });

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
        let show_left = self.show_left;
        let show_right = self.show_left.map(|b| !*b);
        div()
            .fill(
                Switch::new()
                    .push(crate::widgets::switch::Case::new(show_left, || div().class("left").text("LEFT")).cache(true))
                    .push(crate::widgets::switch::Case::new(show_right, || div().class("right").text("RIGHT"))),
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
        let count = self.count;
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
        let seed = self.seed;
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
        seed,
        result_handle: handle.clone(),
    });

    let cell = (*handle.borrow()).expect("resource cell was published");
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
        count,
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
    let holder = make_holder().mount(SwitchHostWidget { show_left });

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
