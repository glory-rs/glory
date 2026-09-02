//! SSR-driven snapshot tests for the keyed list / branch / loader widgets.
//!
//! These tests exercise the actual mount → patch → DOM-tree pipeline against
//! the in-memory `Node` backend, so they catch regressions in both the
//! widget logic and the surrounding scheduler/scope plumbing.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::config::GloryConfig;
use crate::reflow::{Cage, Revisable, effect_in, resource_hydratable_in, resource_in};
use crate::scope::SuspenseBoundary as SuspenseBoundaryHandle;
use crate::web::holders::ServerHolder;
use crate::web::widgets::{
    button, div, form, head_mixin, input, label, li, link, math as math_widgets, meta, option, script, select, style, svg as svg_widgets, textarea,
    title, ul,
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

/// A keyed list living inside a `Suspense` body, so the list can be mutated
/// while the boundary is showing its fallback.
#[derive(Debug)]
struct SuspendedListWidget {
    items: Cage<Vec<String>>,
    boundary: Rc<RefCell<Option<SuspenseBoundaryHandle>>>,
}

impl Widget for SuspendedListWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let boundary = ctx.suspense_boundary.expect("SuspendedListWidget must be under Suspense");
        boundary.start();
        *self.boundary.borrow_mut() = Some(boundary);
        ul().fill(Each::from_vec(self.items, |s| s.clone(), |s| li().text(s.clone())))
            .show_in(ctx);
    }
}

#[test]
fn rows_mutated_while_suspended_do_not_duplicate_or_leak_into_the_fallback() {
    let items = Cage::new(vec!["a".to_owned(), "b".to_owned()]);
    let boundary = Rc::new(RefCell::new(None));
    let holder = make_holder().mount(Suspense::new(
        SuspendedListWidget {
            items,
            boundary: boundary.clone(),
        },
        |ctx| {
            div().class("fallback").text("loading").show_in(ctx);
        },
    ));

    // Pending: only the fallback is in the document.
    let html = render_html(&holder);
    assert!(html.contains("loading"), "{html}");
    assert!(!html.contains("<li"), "suspended body must not render rows: {html}");

    // Remove "b" and add "c" while the body is hidden. `Each::patch` still
    // runs (its subscriptions are live), so it detaches and attaches rows on
    // a scope that is out of the document.
    items.revise(|mut value| {
        *value = vec!["a".to_owned(), "c".to_owned()];
    });

    // Regression: `attach_child` used to insert the new row's node anyway,
    // so hidden content appeared alongside the fallback.
    let html = render_html(&holder);
    assert!(!html.contains("<li"), "hidden rows must not be inserted next to the fallback: {html}");

    boundary.borrow().expect("suspense boundary captured").finish();

    // Regression: `detach_child` bailed out on unattached views, so "b" stayed
    // in `child_views` and the default `flood` re-attached it on resume.
    let rows = each_html_items(&holder);
    assert_eq!(rows, vec!["a".to_owned(), "c".to_owned()], "resumed list must match the current items");
}

#[derive(Debug)]
struct StreamingResourceWidget;

impl Widget for StreamingResourceWidget {
    fn build(&mut self, ctx: &mut Scope) {
        // An async resource keeps the boundary pending until its (deferred)
        // future resolves during the streaming drain.
        let _res = resource_in(ctx, || async { 1_i32 });
        div().class("body").text("ready").show_in(ctx);
    }
}

#[test]
fn streaming_suspense_emits_placeholder_then_patch() {
    let holder = ServerHolder::new_streaming(GloryConfig::default(), "/").mount(Suspense::new(StreamingResourceWidget, |ctx| {
        div().class("fallback").text("loading").show_in(ctx);
    }));

    let html = holder.render_string();

    // The initial shell collapses the pending boundary to a placeholder
    // marker wrapping the fallback.
    let marker = html.find(r#"data-glory-placeholder="gly-suspense-0""#).expect(&html);
    assert!(html.contains("loading"), "{html}");

    // The resolved body streams in afterwards as an out-of-order patch the
    // client runtime applies.
    let patch = html.find(r#"data-glory-placeholder-patch="gly-suspense-0""#).expect(&html);
    assert!(html.contains("ready"), "{html}");
    assert!(html.contains(r#"patchFromTemplate("gly-suspense-0")"#), "{html}");

    // Placeholder is flushed before its patch.
    assert!(marker < patch, "placeholder must precede patch: {html}");

    // The internal wrapper element is never serialized.
    assert!(!html.contains("<glory-suspense"), "{html}");
}

#[test]
fn streaming_holder_without_suspense_matches_blocking_render() {
    let streamed = ServerHolder::new_streaming(GloryConfig::default(), "/")
        .mount(StreamWidget)
        .render_string();
    let blocking = make_holder().mount(StreamWidget).render_string();
    assert_eq!(streamed, blocking);
}

#[derive(Debug)]
struct StreamWidget;

impl Widget for StreamWidget {
    fn build(&mut self, ctx: &mut Scope) {
        div().class("plain").text("static").show_in(ctx);
    }
}

#[derive(Debug)]
struct HydratableResourceWidget;

impl Widget for HydratableResourceWidget {
    fn build(&mut self, ctx: &mut Scope) {
        // The resolved value (42) must end up in the hydration payload so the
        // client can skip the refetch.
        let _res = resource_hydratable_in(ctx, || async { 42_i32 });
        div().class("body").text("ready").show_in(ctx);
    }
}

#[test]
fn streaming_hydratable_resource_embeds_value_payload() {
    let holder = ServerHolder::new_streaming(GloryConfig::default(), "/").mount(Suspense::new(HydratableResourceWidget, |ctx| {
        div().class("fallback").text("loading").show_in(ctx);
    }));

    let html = holder.render_string();

    // The resolved value is streamed alongside the body patch.
    assert!(html.contains("data-glory-placeholder-patch="), "{html}");
    assert!(html.contains("window.__gloryResource=Object.assign"), "{html}");
    // The value is serialized under the resource's stable token (42 appears
    // only in the payload, never in the body text "ready").
    assert!(html.contains(":42}"), "{html}");
}

#[test]
fn blocking_hydratable_resource_embeds_value_payload() {
    let holder = make_holder().mount(HydratableResourceWidget);
    let html = holder.render_string();

    // Even the non-streaming render embeds the value so hydration skips the
    // refetch.
    assert!(html.contains("window.__gloryResource"), "{html}");
    assert!(html.contains(":42}"), "{html}");
    // The body renders fully resolved.
    assert!(html.contains("ready"), "{html}");
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

/// A widget whose `patch` attaches a child that panics during `build`.
///
/// `Scope::attach_child` re-enters `reflow::batch`, so the panic unwinds out of
/// a *nested* batch — the path that used to leave `BATCHING` latched on.
#[derive(Debug)]
struct PanicOnPatchAttach {
    trigger: Cage<bool>,
}

impl Widget for PanicOnPatchAttach {
    fn build(&mut self, ctx: &mut Scope) {
        self.trigger.bind_view(ctx.view_id());
        div().text("ready").show_in(ctx);
    }

    fn patch(&mut self, ctx: &mut Scope) {
        PanicBuildWidget.show_in(ctx);
    }
}

#[derive(Debug)]
struct NestedBatchPanicHost {
    trigger: Cage<bool>,
    counter: Cage<i64>,
}

impl Widget for NestedBatchPanicHost {
    fn build(&mut self, ctx: &mut Scope) {
        ErrorBoundary::new(PanicOnPatchAttach { trigger: self.trigger }, |error, ctx| {
            div().class("error").text(error.message().to_owned()).show_in(ctx);
        })
        .show_in(ctx);
        div().class("counter").text(self.counter).show_in(ctx);
    }
}

#[test]
fn caught_panic_inside_nested_batch_does_not_freeze_the_scheduler() {
    let trigger = Cage::new(false);
    let counter = Cage::new(1);
    let holder = make_holder().mount(NestedBatchPanicHost { trigger, counter });
    assert!(render_html(&holder).contains("ready"));

    trigger.revise(|mut value| *value = true);
    let html = render_html(&holder);
    assert!(html.contains(r#"class="error""#), "boundary should render its fallback: {html}");

    // Regression: the scheduler flags were plain `set(true) ... set(false)`
    // pairs, so a panic unwinding through a nested `batch` left BATCHING (and
    // RUNNING) latched for this holder. `schedule()` then took its
    // `is_batching()` early return forever and every later revise was silently
    // dropped -- the session looked frozen even though the panic was handled.
    counter.revise(|mut value| *value = 42);
    let html = render_html(&holder);
    assert!(html.contains("42"), "scheduler must still process revisions after a caught panic: {html}");
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
    // `<style>` is an HTML raw-text element: the browser does not decode
    // character references inside it. Entity-escaping the CSS (as SSR used to)
    // emitted `.cls &gt; button`, which is not a selector, so the rule never
    // applied until hydration replaced it.
    assert!(html.contains(&format!(".{} > button", scope.class_name())), "{html}");
    assert!(!html.contains("&gt;"), "CSS must not be entity-escaped: {html}");
    assert!(html.contains(r#"<button gly-id="#), "{html}");
}

#[test]
fn duplicate_keys_collapse_to_one_row() {
    // Documented behaviour: "duplicates collapse to a single view".
    let items = Cage::new(vec!["a".to_owned(), "b".to_owned(), "a".to_owned()]);
    let holder = make_holder().mount(EachListWidget { items });
    assert_eq!(each_html_items(&holder), vec!["a".to_owned(), "b".to_owned()]);

    // Regression: a *new* duplicated key built two views and registered only
    // the second. Both rendered, and the first became unreachable, so it
    // survived every later detach.
    items.revise(|mut value| *value = vec!["c".to_owned(), "c".to_owned(), "d".to_owned()]);
    assert_eq!(each_html_items(&holder), vec!["c".to_owned(), "d".to_owned()]);

    // The orphan would show up here, as a stale row nothing can remove.
    items.revise(|mut value| *value = vec!["d".to_owned()]);
    assert_eq!(each_html_items(&holder), vec!["d".to_owned()]);
}

#[derive(Debug)]
struct BranchingBondWidget {
    flag: Cage<bool>,
    a: Cage<i64>,
    b: Cage<i64>,
}

impl Widget for BranchingBondWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let (flag, a, b) = (self.flag, self.a, self.b);
        // Reads `a` or `b` depending on `flag`, with an equality gate.
        let shown = crate::reflow::Bond::new(move || if *flag.get() { *a.get() } else { *b.get() }).with_partial_eq();
        div().class("value").text(shown).show_in(ctx);
    }
}

#[test]
fn a_bond_tracks_the_branch_it_actually_reads() {
    let flag = Cage::new(true);
    let a = Cage::new(1);
    let b = Cage::new(1);
    let holder = make_holder().mount(BranchingBondWidget { flag, a, b });
    assert!(render_html(&holder).contains(">1<"));

    // Switch to the `b` branch. `b == a`, so the equality gate reports "no
    // change" — but the Bond now depends on `b`, not `a`.
    flag.revise(|mut value| *value = false);

    // Regression: new dependencies were only bound when the value changed, so
    // `b` stayed unsubscribed and this write never reached the view.
    b.revise(|mut value| *value = 42);
    let html = render_html(&holder);
    assert!(html.contains(">42<"), "the newly-read dependency must be subscribed: {html}");

    // And the abandoned branch no longer drives the view.
    a.revise(|mut value| *value = 999);
    let html = render_html(&holder);
    assert!(html.contains(">42<"), "dropped dependency must not resurrect the old branch: {html}");
    assert!(!html.contains("999"), "{html}");
}

#[derive(Debug)]
struct StringFillerWidget {
    dynamic: Cage<String>,
}

impl Widget for StringFillerWidget {
    fn build(&mut self, ctx: &mut Scope) {
        div()
            .class("literal")
            .fill("<img src=x onerror=alert(1)>")
            .fill(self.dynamic)
            .show_in(ctx);
        div().class("explicit").html("<b>markup</b>").show_in(ctx);
    }
}

#[test]
fn string_fillers_render_as_text_not_markup() {
    let dynamic = Cage::new("<script>alert(2)</script>".to_owned());
    let holder = make_holder().mount(StringFillerWidget { dynamic });
    let html = render_html(&holder);

    // Regression: `IntoFiller` for strings expanded to `span().html(..)`, so
    // `div().fill(user_input)` wrote straight into innerHTML — an XSS sink
    // with nothing in its spelling to warn you.
    assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"), "{html}");
    assert!(!html.contains("<img src=x"), "{html}");
    assert!(html.contains("&lt;script&gt;alert(2)&lt;/script&gt;"), "{html}");

    // Raw markup is still available, but only when asked for by name.
    assert!(html.contains("<b>markup</b>"), "{html}");
}

#[test]
fn reactive_string_fillers_update() {
    // The old impl snapshotted `self.get()` at build time, so the value never
    // changed afterwards even though the read had subscribed the view.
    let dynamic = Cage::new("before".to_owned());
    let holder = make_holder().mount(StringFillerWidget { dynamic });
    assert!(render_html(&holder).contains("before"));

    dynamic.revise(|mut value| *value = "after".to_owned());
    let html = render_html(&holder);
    assert!(html.contains("after"), "{html}");
    assert!(!html.contains("before"), "{html}");
}

#[derive(Debug)]
struct RawTextWidget;

impl Widget for RawTextWidget {
    fn build(&mut self, ctx: &mut Scope) {
        style().text("a[data-x=\"1\"] > b { content: \"</style>\"; }").show_in(ctx);
        script().text("if (a < b && c > d) { alert(\"</script>\"); }").show_in(ctx);
        div().text("plain < text & more").show_in(ctx);
    }
}

#[test]
fn raw_text_elements_keep_their_source_but_cannot_be_closed_early() {
    let holder = make_holder().mount(RawTextWidget);
    let html = render_html(&holder);

    // Operators survive verbatim in CSS and JS.
    assert!(html.contains("a[data-x=\"1\"] > b"), "{html}");
    assert!(html.contains("a < b && c > d"), "{html}");

    // ... but a closing tag inside the text cannot terminate the element.
    assert!(!html.contains("</style>\"; }"), "style must not be closed early: {html}");
    assert!(!html.contains("</script>\"); }"), "script must not be closed early: {html}");
    assert!(html.contains(r"<\/style>"), "{html}");
    assert!(html.contains(r"<\/script>"), "{html}");

    // Ordinary elements still escape their text.
    assert!(html.contains("plain &lt; text &amp; more"), "{html}");
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
