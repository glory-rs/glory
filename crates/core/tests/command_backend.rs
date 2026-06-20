//! M6 acceptance: full widget-tree → command-stream → event → patch loop,
//! entirely headless. Run with:
//!
//! ```text
//! cargo test -p glory-core --features backend-command --test command_backend
//! ```

#![cfg(feature = "backend-command")]

use glory_core::reflow::Cage;
use glory_core::renderer::command_dom::{CommandDom, ROOT_ID};
use glory_core::renderer::{Command, EventData, TargetData};
use glory_core::web::events;
use glory_core::web::holders::CommandHolder;
use glory_core::web::widgets::{button, div, form, input, label, li, math as math_widgets, option, select, svg as svg_widgets, textarea, ul};
use glory_core::widgets::Each;
use glory_core::{Holder, Scope, Widget};

#[derive(Debug)]
struct Counter {
    value: Cage<i64>,
}

impl Widget for Counter {
    fn build(&mut self, ctx: &mut Scope) {
        let value = self.value.clone();
        let increase = move |_| {
            value.revise(|mut value| *value += 1);
        };
        let value = self.value.clone();
        let set_from_input = move |ev: glory_core::renderer::EventData| {
            let v = glory_core::web::helpers::event_target_value(&ev).parse::<i64>().unwrap_or_default();
            value.revise(|mut value| *value = v);
        };
        div()
            .fill(button().attr("data-role", "inc").text(self.value.clone()).on(events::click, increase))
            .fill(input().attr("data-role", "in").on(events::input, set_from_input))
            .show_in(ctx);
    }
}

/// Finds the node id of the element carrying `data-role="{role}"`.
fn node_id_by_role(dom: &CommandDom, batch: &[Command], role: &str) -> u64 {
    batch
        .iter()
        .find_map(|command| match command {
            Command::SetAttribute { id, name, value } if name == "data-role" && value == role => Some(*id),
            _ => None,
        })
        .or_else(|| {
            let _ = dom;
            None
        })
        .unwrap_or_else(|| panic!("no node with data-role={role}"))
}

#[test]
fn counter_full_loop_mount_click_patch() {
    let holder = CommandHolder::new().mount(Counter { value: Cage::new(0) });

    // Batch 1: full initial build.
    let batch = holder.take_batch();
    let mut dom = CommandDom::new();
    dom.apply_batch(&batch);

    let button_id = node_id_by_role(&dom, &batch, "inc");
    let input_id = node_id_by_role(&dom, &batch, "in");
    let button_node = dom.node(button_id).expect("button exists");
    assert_eq!(button_node.text.as_deref(), Some("0"));
    assert!(button_node.listeners.contains("click"), "AttachEvent must reach the consumer");

    // Click → handler revises the cage → patch lands in batch 2.
    assert!(holder.dispatch_event(EventData::new("click", button_id)));
    let batch2 = holder.take_batch();
    assert!(!batch2.is_empty(), "click must produce a patch batch");
    dom.apply_batch(&batch2);
    assert_eq!(dom.node(button_id).unwrap().text.as_deref(), Some("1"));

    // Input event carries the serialized target value across the "IPC" boundary.
    let mut input_event = EventData::new("input", input_id);
    input_event.target = Some(TargetData {
        value: Some("42".into()),
        checked: None,
    });
    assert!(holder.dispatch_event(input_event));
    dom.apply_batch(&holder.take_batch());
    assert_eq!(dom.node(button_id).unwrap().text.as_deref(), Some("42"));

    // Unknown node: normal in-flight race, must be a no-op.
    assert!(!holder.dispatch_event(EventData::new("click", 9999)));
}

#[test]
fn mounted_lifecycle_event_runs_before_initial_batch_is_drained() {
    #[derive(Debug)]
    struct MountedWidget {
        text: Cage<&'static str>,
    }

    impl Widget for MountedWidget {
        fn build(&mut self, ctx: &mut Scope) {
            let text = self.text.clone();
            div()
                .on(events::mounted, move |_| text.revise(|mut text| *text = "mounted"))
                .text(self.text.clone())
                .show_in(ctx);
        }
    }

    let holder = CommandHolder::new().mount(MountedWidget { text: Cage::new("initial") });
    let batch = holder.take_batch();
    let mut dom = CommandDom::new();
    dom.apply_batch(&batch);

    assert!(
        batch
            .iter()
            .any(|command| matches!(command, Command::AttachEvent { name, .. } if name == "mounted")),
        "mounted listener must be visible to command consumers: {batch:?}"
    );
    assert_eq!(dom.texts_of("div"), vec!["mounted"]);
}

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

fn strs(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| s.to_string()).collect()
}

/// Conformance counterpart of the SSR snapshot tests: the same keyed-list
/// scenarios must produce the same final order when the command stream is
/// replayed through the reference interpreter.
#[test]
fn each_reorder_conformance_via_command_stream() {
    let items = Cage::new(strs(&["a", "b", "c", "d"]));
    let holder = CommandHolder::new().mount(EachListWidget { items: items.clone() });

    let mut dom = CommandDom::new();
    dom.apply_batch(&holder.take_batch());
    assert_eq!(dom.texts_of("li"), strs(&["a", "b", "c", "d"]));

    for target in [
        strs(&["d", "c", "b", "a"]),           // reverse
        strs(&["c", "a", "d", "b"]),           // shuffle
        strs(&["x", "c", "a", "d", "b", "y"]), // head+tail insert
        strs(&["c", "d"]),                     // remove
        strs(&[]),                             // clear
        strs(&["a", "b", "c"]),                // rebuild after clear
    ] {
        let next = target.clone();
        holder.update(|| {
            items.revise(|mut value| *value = next.clone());
        });
        dom.apply_batch(&holder.take_batch());
        assert_eq!(dom.texts_of("li"), target);
    }
}

#[test]
fn removed_rows_release_event_handlers() {
    #[derive(Debug)]
    struct ButtonRows {
        items: Cage<Vec<String>>,
    }
    impl Widget for ButtonRows {
        fn build(&mut self, ctx: &mut Scope) {
            ul().fill(Each::from_vec(
                self.items.clone(),
                |s| s.clone(),
                |s| li().fill(button().text(s.clone()).on(events::click, |_| {})),
            ))
            .show_in(ctx);
        }
    }

    let items = Cage::new(strs(&["a", "b"]));
    let holder = CommandHolder::new().mount(ButtonRows { items: items.clone() });
    let batch = holder.take_batch();
    let attached: Vec<u64> = batch
        .iter()
        .filter_map(|c| match c {
            Command::AttachEvent { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(attached.len(), 2);

    holder.update(|| items.revise(|mut value| value.truncate(1)));
    let batch2 = holder.take_batch();
    let detached: Vec<u64> = batch2
        .iter()
        .filter_map(|c| match c {
            Command::DetachEvent { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(detached.len(), 1, "dropping a row must release its handler: {batch2:?}");
    // The released node's handler is gone from the registry too.
    assert!(!holder.dispatch_event(EventData::new("click", detached[0])));
}

#[test]
fn node_query_round_trips_through_consumer() {
    use glory_core::renderer::{NodeQuery, QueryError, QueryValue};

    let holder = CommandHolder::new().mount(Counter { value: Cage::new(0) });
    let mut dom = CommandDom::new();
    let batch = holder.take_batch();
    dom.apply_batch(&batch);
    let input_id = node_id_by_role(&dom, &batch, "in");

    // Give the input a property value the consumer can answer with.
    dom.apply(&Command::SetProperty {
        id: input_id,
        name: "value".into(),
        value: "live".into(),
    });

    // Issue the query; the request crosses as a `Command::Query`.
    let future = holder.renderer().queue().query(input_id, NodeQuery::Value);
    let query_batch = holder.take_batch();
    assert!(matches!(query_batch[..], [Command::Query { id, .. }] if id == input_id));
    dom.apply_batch(&query_batch);

    // Consumer answers asynchronously; resolving wakes the future.
    let responses = dom.take_query_responses();
    assert_eq!(responses.len(), 1);
    assert!(holder.resolve_query(responses[0].clone()));
    assert_eq!(futures::executor::block_on(future), Ok(QueryValue::Value("live".into())));

    // Unknown node → NodeGone.
    let future = holder.renderer().queue().query(9999, NodeQuery::Value);
    dom.apply_batch(&holder.take_batch());
    let responses = dom.take_query_responses();
    assert!(holder.resolve_query(responses[0].clone()));
    assert_eq!(futures::executor::block_on(future), Err(QueryError::NodeGone));
}

#[derive(Debug)]
struct MarkupSurface;

impl Widget for MarkupSurface {
    fn build(&mut self, ctx: &mut Scope) {
        div()
            .fill(
                form()
                    .attr("method", "post")
                    .fill(label().attr("for", "title").text("Title"))
                    .fill(input().id("title").attr("name", "title").attr("value", "Buy milk"))
                    .fill(input().attr("type", "checkbox").attr("checked", true))
                    .fill(select().fill(option().attr("value", "high").text("High")))
                    .fill(textarea().text("Bring bags"))
                    .fill(button().attr("type", "submit").text("Save")),
            )
            .fill(
                svg_widgets::svg()
                    .attr("viewBox", "0 0 10 10")
                    .fill(svg_widgets::title().text("Badge"))
                    .fill(svg_widgets::circle().attr("cx", "5").attr("cy", "5").attr("r", "4")),
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
            .show_in(ctx);
    }
}

#[test]
fn markup_surface_conformance_via_command_stream() {
    let holder = CommandHolder::new().mount(MarkupSurface);
    let mut dom = CommandDom::new();
    dom.apply_batch(&holder.take_batch());
    let html = dom.inner_html(ROOT_ID);

    assert!(html.contains("<form"), "{html}");
    assert!(html.contains(r#"method="post""#), "{html}");
    assert!(html.contains(r#"name="title""#), "{html}");
    assert!(html.contains("Bring bags"), "{html}");
    assert!(html.contains("<svg"), "{html}");
    assert!(html.contains(r#"viewBox="0 0 10 10""#), "{html}");
    assert!(html.contains("<circle"), "{html}");
    assert!(html.contains("<math"), "{html}");
    assert!(html.contains("<mfrac"), "{html}");
}
