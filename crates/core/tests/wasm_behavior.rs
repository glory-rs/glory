//! Headless-browser **behavior** tests for the CSR runtime.
//!
//! Where [`wasm_csr_smoke`](./wasm_csr_smoke.rs) only proves the CSR path
//! compiles/links and a couple of DOM relationships hold, this suite drives
//! the real reactive runtime in a browser and asserts on the *resulting DOM*:
//! reactive text/attribute updates, derived (`Bond`) updates, event handlers
//! mutating a `Cage` and the DOM reflecting it, keyed-list (`Each`)
//! insert/remove/reorder, conditional (`Switch`) toggling, and mount/unmount
//! subtree lifecycle.
//!
//! These run via `wasm-bindgen-test` against a headless browser
//! (Chrome/Firefox) **in CI** — there is no local wasm runner, so locally the
//! contract is only that they compile for `wasm32-unknown-unknown`. Execution
//! is the CI browser runner's job.
//!
//! Run with:
//!
//! ```sh
//! cargo install wasm-bindgen-cli
//! cargo test -p glory-core --target wasm32-unknown-unknown \
//!     --no-default-features --features web-csr \
//!     --test wasm_behavior
//! ```
//!
//! Reactivity note: `Cage::revise` flushes the scheduler synchronously when no
//! outer batch/run is active (see `reflow::scheduler::run`). Inside an event
//! handler the runtime wraps the handler in `reflow::batch`, so the flush
//! happens once when the handler returns. In both cases the DOM is already
//! updated by the time control returns to the test, so assertions can read the
//! DOM immediately after a `revise()` / `click()` without awaiting a tick.

#![cfg(all(target_arch = "wasm32", feature = "web-csr"))]

use glory_core::reflow::{Bond, Cage};
use glory_core::web::events;
use glory_core::web::holders::BrowserHolder;
use glory_core::web::widgets::{button, div, li, span, ul};
use glory_core::widgets::switch::Case;
use glory_core::widgets::{Each, Switch};
use glory_core::{Holder, Scope, Widget};
use wasm_bindgen::{JsCast, UnwrapThrowExt};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ---------------------------------------------------------------------------
// Test harness helpers
// ---------------------------------------------------------------------------

/// Create a fresh detached host `<div>` appended to `<body>`, mount `widget`
/// into it via a `BrowserHolder`, and return `(holder, host)`. The host stays
/// in the live document so `query_selector` / event dispatch behave exactly as
/// in a real page. Call [`teardown`] (or `host.remove()`) when done.
fn mount_into_host(widget: impl Widget) -> (BrowserHolder, web_sys::Element) {
    let document = glory_core::web::document();
    let host = document.create_element("div").unwrap_throw();
    document.body().unwrap_throw().append_child(&host).unwrap_throw();
    let holder = BrowserHolder::with_host_node(&host).mount(widget);
    (holder, host)
}

fn teardown(host: &web_sys::Element) {
    host.remove();
}

fn text_of(host: &web_sys::Element, selector: &str) -> Option<String> {
    host.query_selector(selector)
        .unwrap_throw()
        .map(|el| el.text_content().unwrap_or_default())
}

fn require(host: &web_sys::Element, selector: &str) -> web_sys::HtmlElement {
    host.query_selector(selector)
        .unwrap_throw()
        .unwrap_throw()
        .unchecked_into::<web_sys::HtmlElement>()
}

/// Ordered text content of every `<li>` directly inside the matched `<ul>`.
fn li_texts(host: &web_sys::Element, list_selector: &str) -> Vec<String> {
    let list = host.query_selector(list_selector).unwrap_throw().unwrap_throw();
    let items = list.query_selector_all("li").unwrap_throw();
    (0..items.length())
        .map(|i| {
            let el = items.item(i).unwrap_throw().unchecked_into::<web_sys::Element>();
            el.text_content().unwrap_or_default()
        })
        .collect()
}

// ===========================================================================
// 1. Reactivity: Cage -> bound text node updates
// ===========================================================================

#[derive(Debug)]
struct ReactiveTextWidget {
    label: Cage<String>,
}
impl Widget for ReactiveTextWidget {
    fn build(&mut self, ctx: &mut Scope) {
        div().id("rt-root").fill(span().id("rt-label").text(self.label)).show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn cage_revise_updates_bound_text_node() {
    let label = Cage::new("initial".to_string());
    let (_holder, host) = mount_into_host(ReactiveTextWidget { label });

    assert_eq!(text_of(&host, "#rt-label").as_deref(), Some("initial"));

    label.revise(|mut v| *v = "updated".to_string());
    assert_eq!(text_of(&host, "#rt-label").as_deref(), Some("updated"));

    label.revise(|mut v| *v = "again".to_string());
    assert_eq!(text_of(&host, "#rt-label").as_deref(), Some("again"));

    teardown(&host);
}

// ===========================================================================
// 2. Reactivity: Cage -> bound attribute updates
// ===========================================================================

#[derive(Debug)]
struct ReactiveAttrWidget {
    title: Cage<String>,
}
impl Widget for ReactiveAttrWidget {
    fn build(&mut self, ctx: &mut Scope) {
        div().id("ra-node").attr("data-title", self.title).text("x").show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn cage_revise_updates_bound_attribute() {
    let title = Cage::new("one".to_string());
    let (_holder, host) = mount_into_host(ReactiveAttrWidget { title });

    let node = require(&host, "#ra-node");
    assert_eq!(node.get_attribute("data-title").as_deref(), Some("one"));

    title.revise(|mut v| *v = "two".to_string());
    assert_eq!(node.get_attribute("data-title").as_deref(), Some("two"));

    teardown(&host);
}

// ===========================================================================
// 3. Reactivity: Bond derived from a Cage updates when the source changes
// ===========================================================================

#[derive(Debug)]
struct BondWidget {
    count: Cage<i32>,
}
impl Widget for BondWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let count = self.count;
        let doubled: Bond<String> = Bond::new(move || format!("={}", *count.get() * 2));
        div().id("bond-out").text(doubled).show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn bond_derived_text_updates_on_source_revise() {
    let count = Cage::new(3_i32);
    let (_holder, host) = mount_into_host(BondWidget { count });

    assert_eq!(text_of(&host, "#bond-out").as_deref(), Some("=6"));

    count.revise(|mut v| *v = 10);
    assert_eq!(text_of(&host, "#bond-out").as_deref(), Some("=20"));

    teardown(&host);
}

// ===========================================================================
// 4. Events: click -> handler revises Cage -> DOM updates
// ===========================================================================

#[derive(Debug)]
struct CounterWidget {
    count: Cage<i32>,
}
impl Widget for CounterWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let count = self.count;
        let label: Bond<String> = Bond::new(move || count.get().to_string());
        div()
            .id("counter-root")
            .fill(span().id("counter-value").text(label))
            .fill(
                button()
                    .id("counter-inc")
                    .text("+")
                    .on(events::click, move |_| count.revise(|mut v| *v += 1)),
            )
            .show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn click_handler_revises_cage_and_updates_dom() {
    let count = Cage::new(0_i32);
    let (_holder, host) = mount_into_host(CounterWidget { count });

    assert_eq!(text_of(&host, "#counter-value").as_deref(), Some("0"));

    let inc = require(&host, "#counter-inc");
    inc.click();
    assert_eq!(text_of(&host, "#counter-value").as_deref(), Some("1"));
    inc.click();
    inc.click();
    assert_eq!(text_of(&host, "#counter-value").as_deref(), Some("3"));
    assert_eq!(*count.get_untracked(), 3);

    teardown(&host);
}

// ===========================================================================
// 5. Events: delegated bubbling path — dispatch a real `click` Event that
//    bubbles up from a deep descendant to the listener-bearing element.
// ===========================================================================

#[derive(Debug)]
struct DelegatedWidget {
    hits: Cage<i32>,
}
impl Widget for DelegatedWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let hits = self.hits;
        // `click` bubbles, so the runtime registers it on the delegated path.
        button()
            .id("deleg-button")
            .on(events::click, move |_| hits.revise(|mut v| *v += 1))
            .fill(span().id("deleg-inner").text("inner"))
            .show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn delegated_click_bubbles_from_descendant() {
    let hits = Cage::new(0_i32);
    let (_holder, host) = mount_into_host(DelegatedWidget { hits });

    // `HtmlElement::click()` dispatches a real, bubbling `MouseEvent` from the
    // *inner* span; it must bubble up to the button's (delegated) listener.
    let inner = require(&host, "#deleg-inner");
    inner.click();

    assert_eq!(*hits.get_untracked(), 1, "delegated click should fire once via bubbling");

    teardown(&host);
}

// ===========================================================================
// 6-9. Lists: Each insert / remove / reorder keep DOM child order correct
// ===========================================================================

#[derive(Debug)]
struct EachWidget {
    items: Cage<Vec<String>>,
}
impl Widget for EachWidget {
    fn build(&mut self, ctx: &mut Scope) {
        ul().id("each-list")
            .fill(Each::from_vec(self.items, |s| s.clone(), |s| li().text(s.clone())))
            .show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn each_initial_render_order() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let (_holder, host) = mount_into_host(EachWidget { items });
    assert_eq!(li_texts(&host, "#each-list"), vec!["a", "b", "c"]);
    teardown(&host);
}

#[wasm_bindgen_test]
fn each_append_and_prepend_updates_dom() {
    let items = Cage::new(vec!["b".to_string()]);
    let (_holder, host) = mount_into_host(EachWidget { items });

    items.revise(|mut v| v.push("c".to_string()));
    assert_eq!(li_texts(&host, "#each-list"), vec!["b", "c"]);

    items.revise(|mut v| v.insert(0, "a".to_string()));
    assert_eq!(li_texts(&host, "#each-list"), vec!["a", "b", "c"]);

    teardown(&host);
}

#[wasm_bindgen_test]
fn each_remove_middle_updates_dom() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let (_holder, host) = mount_into_host(EachWidget { items });

    items.revise(|mut v| {
        v.remove(1);
    });
    assert_eq!(li_texts(&host, "#each-list"), vec!["a", "c"]);

    teardown(&host);
}

#[wasm_bindgen_test]
fn each_reorder_preserves_keys_in_dom() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()]);
    let (_holder, host) = mount_into_host(EachWidget { items });

    // Reverse — every key is retained but order flips.
    items.revise(|mut v| v.reverse());
    assert_eq!(li_texts(&host, "#each-list"), vec!["d", "c", "b", "a"]);

    // Arbitrary shuffle of the same keys.
    items.revise(|mut v| {
        *v = vec!["b".to_string(), "d".to_string(), "a".to_string(), "c".to_string()];
    });
    assert_eq!(li_texts(&host, "#each-list"), vec!["b", "d", "a", "c"]);

    teardown(&host);
}

#[wasm_bindgen_test]
fn each_clear_removes_all_children() {
    let items = Cage::new(vec!["a".to_string(), "b".to_string()]);
    let (_holder, host) = mount_into_host(EachWidget { items });
    assert_eq!(li_texts(&host, "#each-list").len(), 2);

    items.revise(|mut v| v.clear());
    assert!(li_texts(&host, "#each-list").is_empty());

    teardown(&host);
}

// ===========================================================================
// 10-11. Conditional: Switch toggles between cases; node presence flips.
// ===========================================================================

#[derive(Debug)]
struct SwitchWidget {
    show_left: Cage<bool>,
}
impl Widget for SwitchWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let show_left = self.show_left;
        let show_right = self.show_left.map(|b| !*b);
        div()
            .id("switch-root")
            .fill(
                Switch::new()
                    .push(Case::new(show_left, || div().class("left").text("LEFT")))
                    .push(Case::new(show_right, || div().class("right").text("RIGHT"))),
            )
            .show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn switch_shows_active_case_only() {
    let show_left = Cage::new(true);
    let (_holder, host) = mount_into_host(SwitchWidget { show_left });

    assert!(host.query_selector(".left").unwrap_throw().is_some());
    assert!(host.query_selector(".right").unwrap_throw().is_none());

    teardown(&host);
}

#[wasm_bindgen_test]
fn switch_toggles_dom_subtree_on_revise() {
    let show_left = Cage::new(true);
    let (_holder, host) = mount_into_host(SwitchWidget { show_left });

    show_left.revise(|mut v| *v = false);
    assert!(host.query_selector(".left").unwrap_throw().is_none(), "LEFT removed");
    assert_eq!(text_of(&host, ".right").as_deref(), Some("RIGHT"));

    // Flip back: RIGHT goes away, LEFT returns.
    show_left.revise(|mut v| *v = true);
    assert_eq!(text_of(&host, ".left").as_deref(), Some("LEFT"));
    assert!(host.query_selector(".right").unwrap_throw().is_none(), "RIGHT removed");

    teardown(&host);
}

// ===========================================================================
// 12. Conditional via Each-as-toggle: a count of 0 vs 1 element toggles a node
//     in/out of the live DOM (exercises mount/unmount of a child subtree).
// ===========================================================================

#[derive(Debug)]
struct ToggleWidget {
    shown: Cage<bool>,
}
impl Widget for ToggleWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let shown = self.shown;
        // Map the bool into a 0-or-1 element list; `Each` mounts/unmounts the
        // single child as the flag flips.
        let items: Bond<Vec<&'static str>> = Bond::new(move || if *shown.get() { vec!["panel"] } else { vec![] });
        ul().id("toggle-list")
            .fill(Each::from_vec(items, |s| s.to_string(), |s| li().class("panel").text(*s)))
            .show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn child_subtree_mounts_and_unmounts() {
    let shown = Cage::new(false);
    let (_holder, host) = mount_into_host(ToggleWidget { shown });

    assert!(host.query_selector(".panel").unwrap_throw().is_none());

    shown.revise(|mut v| *v = true);
    assert!(host.query_selector(".panel").unwrap_throw().is_some(), "panel mounted");
    assert_eq!(li_texts(&host, "#toggle-list"), vec!["panel"]);

    shown.revise(|mut v| *v = false);
    assert!(host.query_selector(".panel").unwrap_throw().is_none(), "panel unmounted");
    assert!(li_texts(&host, "#toggle-list").is_empty());

    teardown(&host);
}

// ===========================================================================
// 13. Lifecycle: detaching the whole mounted subtree removes its nodes from
//     the live document (host removal tears down the rendered tree).
// ===========================================================================

#[derive(Debug)]
struct LifecycleWidget;
impl Widget for LifecycleWidget {
    fn build(&mut self, ctx: &mut Scope) {
        div().id("life-root").fill(span().id("life-child").text("alive")).show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn mounted_subtree_present_then_removed() {
    let document = glory_core::web::document();
    let (_holder, host) = mount_into_host(LifecycleWidget);

    // Present in the live document while mounted.
    assert!(document.query_selector("#life-child").unwrap_throw().is_some());
    assert_eq!(text_of(&host, "#life-child").as_deref(), Some("alive"));

    // Removing the host detaches the entire rendered subtree from the document.
    teardown(&host);
    assert!(document.query_selector("#life-root").unwrap_throw().is_none());
    assert!(document.query_selector("#life-child").unwrap_throw().is_none());
}

// ===========================================================================
// 14. Reactivity: multiple independent subscribers to the same Cage all update.
// ===========================================================================

#[derive(Debug)]
struct MultiSubWidget {
    value: Cage<String>,
}
impl Widget for MultiSubWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let value = self.value;
        let upper: Bond<String> = Bond::new(move || value.get().to_uppercase());
        div()
            .id("multi-root")
            .fill(span().id("multi-raw").text(self.value))
            .fill(span().id("multi-upper").text(upper))
            .show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn multiple_subscribers_update_together() {
    let value = Cage::new("hi".to_string());
    let (_holder, host) = mount_into_host(MultiSubWidget { value });

    assert_eq!(text_of(&host, "#multi-raw").as_deref(), Some("hi"));
    assert_eq!(text_of(&host, "#multi-upper").as_deref(), Some("HI"));

    value.revise(|mut v| *v = "bye".to_string());
    assert_eq!(text_of(&host, "#multi-raw").as_deref(), Some("bye"));
    assert_eq!(text_of(&host, "#multi-upper").as_deref(), Some("BYE"));

    teardown(&host);
}

// ===========================================================================
// 15. Events: toggle handler flips a Cage<bool> driving a Switch, end-to-end
//     (event -> reactive condition -> conditional subtree swap).
// ===========================================================================

#[derive(Debug)]
struct ToggleButtonWidget {
    on: Cage<bool>,
}
impl Widget for ToggleButtonWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let on = self.on;
        let on_case = on;
        let off_case = on.map(|b| !*b);
        div()
            .id("tb-root")
            .fill(
                button()
                    .id("tb-toggle")
                    .text("toggle")
                    .on(events::click, move |_| on.revise(|mut v| *v = !*v)),
            )
            .fill(
                Switch::new()
                    .push(Case::new(on_case, || span().class("state-on").text("ON")))
                    .push(Case::new(off_case, || span().class("state-off").text("OFF"))),
            )
            .show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn event_driven_switch_swaps_subtree() {
    let on = Cage::new(false);
    let (_holder, host) = mount_into_host(ToggleButtonWidget { on });

    assert_eq!(text_of(&host, ".state-off").as_deref(), Some("OFF"));
    assert!(host.query_selector(".state-on").unwrap_throw().is_none());

    let toggle = require(&host, "#tb-toggle");
    toggle.click();
    assert_eq!(text_of(&host, ".state-on").as_deref(), Some("ON"));
    assert!(host.query_selector(".state-off").unwrap_throw().is_none());

    toggle.click();
    assert_eq!(text_of(&host, ".state-off").as_deref(), Some("OFF"));
    assert!(host.query_selector(".state-on").unwrap_throw().is_none());

    teardown(&host);
}
