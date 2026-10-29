use super::*;
use glory_html::view::{self as html, Click, Field, Outlet, Template};
use glory_web::view::{lazy, view as display};
const VALUE: Field = Field::new("value");
const ACTIVE: Field = Field::new("active");
const BODY: Outlet = Outlet::new("body");
const CLICK: Click = Click::new("click");
static TEMPLATE: Template = Template::new(&[html::div(
    &[html::class("base"), html::toggle_class("active", ACTIVE)],
    &[
        html::span(&[], &[html::field(VALUE)]),
        html::button(&[html::on_click(CLICK)], &[html::text("raw")]),
        html::outlet(BODY),
    ],
)]);
fn config(
    value: impl glory_web::view::TextValue + 'static,
    active: impl glory_web::view::BoolValue,
) -> impl Widget {
    display(&TEMPLATE)
        .bind(VALUE, value)
        .class_when(ACTIVE, active)
        .on_click(CLICK, || {})
        .child(BODY, lazy(|| glory_ui::button().label("semantic")))
}
#[test]
fn computed_sources_evaluate_once_per_binding_tick_and_retire_with_the_widget() {
    let reads = Rc::new(Cell::new(0));
    let flags = Rc::new(Cell::new(0));
    let source = Rc::new(RefCell::new(None));
    let (text_reads, flag_reads, captured) = (reads.clone(), flags.clone(), source.clone());
    let mut stream = stream_with(Resources::default(), Config::default(), move |scope, _| {
        let value = scope.cage(0);
        *captured.borrow_mut() = Some(value);
        config(
            glory_web::view::computed(move || {
                text_reads.set(text_reads.get() + 1);
                value.get().to_string()
            }),
            glory_web::view::computed(move || {
                flag_reads.set(flag_reads.get() + 1);
                value.get() > 0
            }),
        )
    })
    .unwrap();
    frame(&mut stream);
    assert_eq!((reads.get(), flags.get()), (1, 1));
    source.borrow().unwrap().revise(|value| *value = 1);
    frame(&mut stream);
    assert_eq!((reads.get(), flags.get()), (2, 2));
    stream.cancel();
    assert!(!source.borrow().unwrap().is_alive());
    assert_eq!((reads.get(), flags.get()), (2, 2));
}
#[test]
fn slot_only_updates_coalesce_without_semantic_mutations_or_new_declarations() {
    let sources = Rc::new(RefCell::new(None));
    let copy = sources.clone();
    let mut stream = stream_with(Resources::default(), Config::default(), move |scope, _| {
        let text = scope.cage("initial".to_owned());
        let active = scope.cage(false);
        *copy.borrow_mut() = Some((text, active));
        config(text, active)
    })
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body.contains("initial") && shell.body.contains("semantic"));
    let initial: serde_json::Value = serde_json::from_str(shell.initial.as_ref().unwrap()).unwrap();
    assert_eq!(
        initial
            .as_array()
            .unwrap()
            .iter()
            .filter(|node| node["shape"]["display"]["kind"] == "root")
            .count(),
        1
    );
    let model = stream.mapper.as_ref().unwrap().clone();
    let applied = model.borrow().receipt().applied;
    let (text, active) = sources.borrow().unwrap();
    text.revise(|value| *value = "superseded".into());
    text.revise(|value| *value = "final <&\0\r\n".into());
    active.revise(|value| *value = true);
    let patch = frame(&mut stream);
    assert_eq!(patch.kind, FrameKind::Patch);
    let data: serde_json::Value = serde_json::from_str(&patch.body).unwrap();
    let ops = data["ops"].as_array().unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0]["op"], "display");
    assert_eq!(ops[0]["values"].as_array().unwrap().len(), 2);
    assert!(!patch.body.contains("superseded"));
    assert!(
        ops[0]["values"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change[1] == "final <&�\n")
    );
    assert_eq!(model.borrow().receipt().applied, applied);
    assert_eq!(stream.edits.borrow().display_stats(), (1, 1));
    let end = frame(&mut stream);
    assert_eq!(end.kind, FrameKind::End);
    assert_eq!(stream.edits.borrow().display_stats(), (0, 0));
    assert_eq!(stream.stats().nodes, 0);
    assert!(text.try_get().is_err());
}
#[test]
fn live_instances_share_declarations_and_retirement_does_not_keep_visited_templates() {
    let source = Rc::new(RefCell::new(None));
    let copy = source.clone();
    let gate = Gate::default();
    let waiting = gate.waiting();
    let mut stream = stream_with(Resources::default(), Config::default(), move |scope, _| {
        let list = scope.keyed_list(vec![1usize, 2], |value| *value).unwrap();
        *copy.borrow_mut() = Some(list);
        scope
            .spawn(async move {
                let _ = waiting.await;
            })
            .unwrap();
        glory_web::view::content(
            glory_web::view::each(list).children(|row| config(*row.key(), false)),
        )
    })
    .unwrap();
    let shell = frame(&mut stream);
    let initial: serde_json::Value = serde_json::from_str(shell.initial.as_ref().unwrap()).unwrap();
    assert_eq!(
        initial
            .as_array()
            .unwrap()
            .iter()
            .filter(|node| node["shape"]["display"]["wire"]
                .as_str()
                .is_some_and(|wire| wire.contains("base")))
            .count(),
        1
    );
    assert_eq!(stream.edits.borrow().display_stats(), (3, 2));
    let list = source.borrow().unwrap();
    list.replace(vec![2, 3]).unwrap();
    let patch = frame(&mut stream);
    let data: serde_json::Value = serde_json::from_str(&patch.body).unwrap();
    assert!(
        data["ops"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["op"] == "create" && op["shape"]["display"]["kind"] == "root")
            .all(|op| op["shape"]["display"]["wire"].is_null())
    );
    list.replace(vec![]).unwrap();
    frame(&mut stream);
    assert_eq!(stream.edits.borrow().display_stats(), (1, 1));
    list.replace(vec![4]).unwrap();
    let patch = frame(&mut stream);
    assert!(patch.body.contains("\\\"base\\\""));
    assert_eq!(stream.edits.borrow().display_stats(), (2, 2));
    stream.cancel();
    assert_eq!(stream.edits.borrow().display_stats(), (0, 0));
    assert_eq!(gate.0.borrow().drops, 1);
    assert_eq!(stream.stats().tasks, 0);
}
#[test]
fn display_payload_limits_cancel_the_request_and_pending_task() {
    let source = Rc::new(RefCell::new(None));
    let copy = source.clone();
    let gate = Gate::default();
    let waiting = gate.waiting();
    let mut stream = stream_with(
        Resources::default(),
        Config {
            chunk_bytes: 16384,
            ..Default::default()
        },
        move |scope, _| {
            let value = scope.cage(String::new());
            *copy.borrow_mut() = Some(value);
            scope
                .spawn(async move {
                    let _ = waiting.await;
                })
                .unwrap();
            config(value, false)
        },
    )
    .unwrap();
    frame(&mut stream);
    let value = source.borrow().unwrap();
    value.revise(|value| *value = "x".repeat(20000));
    assert!(matches!(poll(&mut stream), Poll::Ready(Some(Err(_)))));
    assert_eq!(stream.stats().nodes, 0);
    assert_eq!(stream.stats().tasks, 0);
    assert_eq!(stream.edits.borrow().display_stats(), (0, 0));
    assert!(value.try_get().is_err());
}
