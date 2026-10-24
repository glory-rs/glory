use std::cell::Cell;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Wake, Waker};

use glory_core::{AccessError, Resource, ResourceState};
use glory_ui::{Widget, component, group, list, text, view, when};

use super::*;
#[path = "anchor_tests.rs"]
mod anchor;
#[path = "choice_tests.rs"]
mod choice;
#[path = "collapse_tests.rs"]
mod collapse;
#[path = "command_tests.rs"]
mod command;
#[path = "document_tests.rs"]
mod document;
#[path = "drag_capture_tests.rs"]
mod drag_capture;
#[path = "graphic_tests.rs"]
mod graphic;
#[path = "graphic_transition_tests.rs"]
mod graphic_transition;
#[path = "hints_tests.rs"]
mod hints;
#[path = "image_tests.rs"]
mod image;
#[path = "inert_tests.rs"]
mod inert;
#[path = "menu_tests.rs"]
mod menu;
#[path = "motion_tests.rs"]
mod motion;
#[path = "progress_tests.rs"]
mod progress;
#[path = "sheet_tests.rs"]
mod sheet;

#[test]
fn stream_preflight_rejects_structural_and_grouped_child_changes_before_model_or_queue_updates() {
    use glory_render::{ButtonProps, Change, Descriptor, Patch, Props, Role, TextProps};
    let mut stream = stream_with(Resources::default(), Config::default(), |_, _| {
        component(|scope| {
            let parent = scope.create(Descriptor::new(Props::View))?;
            assert!(
                scope
                    .patch(parent, Patch::Role(Change::Set(Some(Role::Main))))
                    .is_err()
            );
            let parent_children = scope.child_in(parent)?;
            let button =
                parent_children.create(Descriptor::new(Props::Button(ButtonProps::default())))?;
            let children = scope.child_in(button)?;
            let group = children.create(Descriptor::new(Props::Group))?;
            let nested = children.child_in(group)?;
            let mut paragraph = Descriptor::new(Props::Text(TextProps::default()));
            paragraph
                .options
                .insert(glory_html::HtmlTextOptions::default().tag(glory_html::TextTag::P));
            assert_eq!(
                nested.create(paragraph),
                Err(glory_ui::Error::Render(glory_render::Error::InvalidProps(
                    "button text cannot map to an HTML paragraph"
                )))
            );
            let text = nested.create(Descriptor::new(Props::Text(TextProps {
                content: "recovered".into(),
                ..Default::default()
            })))?;
            assert_eq!(text.slot(), group.slot() + 1);
            scope.retain((parent_children, children, nested))
        })
    })
    .unwrap();
    let app = stream.app.as_ref().unwrap();
    assert_eq!(app.stats().nodes, 4);
    assert_eq!(app.stats().pending, 0);
    let shell = frame(&mut stream);
    assert!(shell.body().contains(">recovered</span>"));
    assert!(!shell.body().contains("<main"));
    let end = frame(&mut stream);
    assert!(matches!(end.kind, FrameKind::End));
}

#[test]
fn stream_renderer_obeys_the_same_target_contract_as_synchronous_ssr() {
    let runtime = RuntimeId::new();
    let renderer = Renderer {
        mapper: Rc::new(RefCell::new(Mapper::new(runtime))),
        edits: Rc::new(RefCell::new(edits::Edits::new(128 * 1024))),
        inputs: Vec::new(),
        receipt: Receipt::default(),
    };
    glory_render::conformance::verify(renderer, runtime, false);
}

#[test]
fn stream_renderer_rejected_direct_batch_keeps_successful_prefix_watermarks() {
    use glory_render::{Change, Descriptor, NodeIds, Patch, Props, Role};
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut renderer = Renderer {
        mapper: Rc::new(RefCell::new(Mapper::new(runtime))),
        edits: Rc::new(RefCell::new(edits::Edits::new(4096))),
        inputs: Vec::new(),
        receipt: Receipt::default(),
    };
    assert!(
        renderer
            .apply(&[
                Mutation::Create {
                    id,
                    node: Box::new(Descriptor::new(Props::View))
                },
                Mutation::Place {
                    id,
                    parent: None,
                    before: None
                },
                Mutation::Patch {
                    id,
                    patch: Patch::Role(Change::Set(Some(Role::Main)))
                },
            ])
            .is_err()
    );
    renderer.request(Query::Bounds(id), QueryToken(1)).unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(1),
            applied: 2,
            result: Err(glory_render::Error::Unsupported(_))
        }]
    ));
    assert_eq!(
        renderer
            .apply(&[Mutation::Patch {
                id,
                patch: Patch::Name(Change::Set(Some("usable".into())))
            }])
            .unwrap()
            .applied,
        3
    );
    renderer.request(Query::Focus(id), QueryToken(2)).unwrap();
    inputs.clear();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            applied: 3,
            result: Err(_),
            ..
        }]
    ));
}

#[derive(Clone, Default)]
struct Gate(Rc<RefCell<GateState>>);
#[derive(Default)]
struct GateState {
    ready: Option<Result<String, String>>,
    waker: Option<Waker>,
    polls: usize,
    drops: usize,
}
struct Waiting(Gate);
impl Future for Waiting {
    type Output = Result<String, String>;
    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut gate = self.0.0.borrow_mut();
        gate.polls += 1;
        if let Some(result) = gate.ready.take() {
            Poll::Ready(result)
        } else {
            gate.waker = Some(context.waker().clone());
            Poll::Pending
        }
    }
}
impl Drop for Waiting {
    fn drop(&mut self) {
        self.0.0.borrow_mut().drops += 1;
    }
}
impl Gate {
    fn waiting(&self) -> Waiting {
        Waiting(self.clone())
    }
    fn resolve(&self, value: Result<String, String>) {
        let waker = {
            let mut gate = self.0.borrow_mut();
            gate.ready = Some(value);
            gate.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}
fn poll(stream: &mut Stream) -> Poll<Option<Result<Frame, Error>>> {
    Pin::new(stream).poll_frame(&mut Context::from_waker(Waker::noop()))
}
fn frame(stream: &mut Stream) -> Frame {
    match poll(stream) {
        Poll::Ready(Some(Ok(frame))) => frame,
        other => panic!("expected frame: {other:?}"),
    }
}
fn label(state: &ResourceState<u64, String, String>) -> String {
    match state {
        ResourceState::Ready { value, .. } => value.to_string(),
        ResourceState::Failed { error, .. } => error.to_string(),
        _ => "pending".into(),
    }
}
fn ui(scope: &Scope, resources: &Resources, a: Gate, b: Gate) -> impl Widget + use<> {
    let a = resources
        .bind(scope.owner(), "a", "v1", || 1_u64, move |_| a.waiting())
        .unwrap();
    let b = resources
        .bind(scope.owner(), "b", "v1", || 2_u64, move |_| b.waiting())
        .unwrap();
    let ready = |resource: Resource<u64, String, String>| {
        resource.map(|state| matches!(state, ResourceState::Ready { .. }))
    };
    view().fill((
        when(ready(a), move || group().fill(text().content(a.map(label))))
            .otherwise(|| group().fill(text().content("waiting a"))),
        when(ready(b), move || group().fill(text().content(b.map(label))))
            .otherwise(|| group().fill(text().content("waiting b"))),
    ))
}

#[test]
fn shell_precedes_loader_polling_and_backpressure_stops_execution() {
    let a = Gate::default();
    let b = Gate::default();
    let resources = Resources::default();
    let mut stream = stream_with(resources.clone(), Config::default(), |scope, resources| {
        ui(scope, resources, a.clone(), b.clone())
    })
    .unwrap();
    assert_eq!(a.0.borrow().polls, 0);
    assert_eq!(b.0.borrow().polls, 0);
    let shell = frame(&mut stream);
    assert!(shell.body().starts_with("<!--gly:stream:2-->"));
    assert_eq!(shell.kind(), FrameKind::Shell);
    assert!(shell.body().contains("waiting a"));
    assert_eq!(a.0.borrow().polls, 0);
    assert!(poll(&mut stream).is_pending()); // Only demand starts the loaders.
    assert_eq!(a.0.borrow().polls, 1);
    assert_eq!(b.0.borrow().polls, 1);
    b.resolve(Ok("second first".into()));
    // I/O completion can wake, but no queued HTML producer renders ahead.
    assert_eq!(b.0.borrow().polls, 1);
    assert_eq!(stream.stats().frames, 1);
    let patch_b = frame(&mut stream);
    assert_eq!(patch_b.kind(), FrameKind::Patch);
    assert!(patch_b.body().contains("second first"));
    assert!(!patch_b.body().contains("waiting a"));
    assert_eq!(a.0.borrow().polls, 1);
    a.resolve(Ok("first last".into()));
    let patch_a = frame(&mut stream);
    assert!(patch_a.body().contains("first last"));
    assert_eq!(patch_a.sequence(), 2);
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    assert_eq!(end.sequence(), 3);
    let data: serde_json::Value = serde_json::from_str(end.body()).unwrap();
    assert_eq!(data["html_version"], glory_html::FORMAT_VERSION);
    assert_eq!(data["resources"]["records"].as_array().unwrap().len(), 2);
    let keys: std::collections::BTreeSet<_> = data["rekey"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| pair[1].as_str().unwrap().parse::<u64>().unwrap())
        .collect();
    assert_eq!(keys.len(), data["rekey"].as_array().unwrap().len());
    assert_eq!(
        keys.into_iter().collect::<Vec<_>>(),
        (0..data["rekey"].as_array().unwrap().len() as u64).collect::<Vec<_>>()
    );
    assert!(matches!(poll(&mut stream), Poll::Ready(None)));
    assert!(resources.is_empty());
    assert_eq!(stream.stats().tasks, 0);
    assert_eq!(stream.stats().nodes, 0);
}

#[test]
fn dropping_or_cancelling_a_pending_body_releases_futures_and_publication_rights() {
    let gate = Gate::default();
    let resources = Resources::default();
    let handle = Rc::new(Cell::new(None));
    let mut stream = stream_with(resources.clone(), Config::default(), |scope, resources| {
        let g = gate.clone();
        let resource = resources
            .bind(scope.owner(), "r", "v1", || 1_u64, move |_| g.waiting())
            .unwrap();
        handle.set(Some(resource));
        text().content(resource.map(label))
    })
    .unwrap();
    frame(&mut stream);
    assert!(poll(&mut stream).is_pending());
    let late = gate.0.borrow().waker.clone().unwrap();
    stream.cancel();
    assert_eq!(gate.0.borrow().drops, 1);
    assert!(resources.is_empty());
    assert!(matches!(
        handle.get().unwrap().try_get(),
        Err(AccessError::Disposed | AccessError::RuntimeGone)
    ));
    late.wake();
    assert!(matches!(poll(&mut stream), Poll::Ready(None)));
    let g = gate.clone();
    let stream = stream_with(resources.clone(), Config::default(), |scope, _| {
        scope
            .spawn(async move {
                let _ = g.waiting().await;
            })
            .unwrap();
        text().content("new body")
    })
    .unwrap();
    // This independent body has not polled its loader; dropping still removes
    // its queued task and invalidates all UI lifetime, without running it.
    assert_eq!(stream.stats().tasks, 1);
    drop(stream);
    assert!(resources.is_empty());
}

#[derive(Default)]
struct Observer(AtomicUsize);
impl Wake for Observer {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn external_thread_wakes_local_request_without_crossing_ui_state() {
    let gate = Gate::default();
    let g = gate.clone();
    let mut stream = stream_with(Resources::default(), Config::default(), |scope, _| {
        let resource = scope.resource(|| 1_u64, move |_| g.waiting());
        text().content(resource.map(label))
    })
    .unwrap();
    frame(&mut stream);
    let observer = Arc::new(Observer::default());
    let waker = Waker::from(observer.clone());
    assert!(
        Pin::new(&mut stream)
            .poll_frame(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let task_waker = {
        let mut gate = gate.0.borrow_mut();
        gate.ready = Some(Ok("thread woke".into()));
        gate.waker.take().unwrap()
    };
    std::thread::spawn(move || task_waker.wake())
        .join()
        .unwrap();
    assert!(observer.0.load(Ordering::SeqCst) > 0);
    assert!(frame(&mut stream).body().contains("thread woke"));
    assert_eq!(frame(&mut stream).kind(), FrameKind::End);
}

#[test]
fn per_tick_budget_preserves_dirty_commits_and_fairly_polls_other_tasks() {
    let mut stream = stream_with(
        Resources::default(),
        Config {
            polls_per_tick: 1,
            ..Config::default()
        },
        |scope, _| {
            let count = scope.cage(0);
            scope
                .spawn(async move {
                    count.revise(|count| *count = 1);
                    std::future::pending::<()>().await;
                })
                .unwrap();
            text().content(count.map(|n| n.to_string()))
        },
    )
    .unwrap();
    frame(&mut stream);
    assert!(poll(&mut stream).is_pending());
    assert!(frame(&mut stream).body().contains("\"value\":\"1\""));
    stream.cancel();
    let mut stream = stream_with(
        Resources::default(),
        Config {
            polls_per_tick: 1,
            ..Config::default()
        },
        |scope, _| {
            scope
                .spawn(std::future::poll_fn(|context| {
                    context.waker().wake_by_ref();
                    Poll::<()>::Pending
                }))
                .unwrap();
            let source =
                scope.resource(|| 1_u64, |_| async { Ok::<_, String>("other task".into()) });
            text().content(source.map(label))
        },
    )
    .unwrap();
    frame(&mut stream);
    let mut found = false;
    for _ in 0..10 {
        if let Poll::Ready(Some(Ok(next))) = poll(&mut stream) {
            found |= next.body().contains("other task");
            break;
        }
    }
    assert!(found, "self-waking producer must not starve another task");
}

#[test]
fn bounded_frames_report_errors_and_release_the_whole_request() {
    let resources = Resources::default();
    let a = Gate::default();
    let b = Gate::default();
    let mut stream = stream_with(
        resources.clone(),
        Config {
            chunk_bytes: 8192,
            ..Config::default()
        },
        |scope, resources| ui(scope, resources, a.clone(), b.clone()),
    )
    .unwrap();
    frame(&mut stream);
    assert!(poll(&mut stream).is_pending());
    a.resolve(Ok("x".repeat(32000)));
    assert!(matches!(poll(&mut stream), Poll::Ready(Some(Err(_)))));
    assert!(resources.is_empty());
    assert_eq!(stream.stats().tasks, 0);
    assert_eq!(b.0.borrow().drops, 1);
    assert!(matches!(poll(&mut stream), Poll::Ready(None)));
    let resources = Resources::default();
    let a = Gate::default();
    let b = Gate::default();
    let result = stream_with(
        resources.clone(),
        Config {
            chunk_bytes: 1,
            ..Config::default()
        },
        |scope, resources| ui(scope, resources, a, b),
    );
    assert!(matches!(result, Err(Error::Limit("shell bytes"))));
    assert!(resources.is_empty());
}

#[test]
fn huge_list_shell_and_owned_requests_remain_window_bounded() {
    let resources = Resources::default();
    let gate = Gate::default();
    let reads = Rc::new(Cell::new(0));
    let mut stream = stream_with(resources.clone(), Config::default(), |_, resources| {
        let reads = reads.clone();
        let resources = resources.clone();
        let gate = gate.clone();
        list(
            100_000_000_usize,
            move |index| {
                reads.set(reads.get() + 1);
                index
            },
            move |_, key| {
                let resources = resources.clone();
                let gate = gate.clone();
                component(move |scope| {
                    let gate = gate.clone();
                    let data = resources
                        .bind(
                            scope.owner(),
                            &format!("row/{key}"),
                            "v1",
                            move || key as u64,
                            move |_| gate.waiting(),
                        )
                        .unwrap();
                    text().content(data.map(label)).show_in(scope)?;
                    Ok(())
                })
            },
        )
        .height(600.0)
        .row_height(24.0)
        .overscan(4)
    })
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().len() < 30_000);
    assert!(reads.get() <= 34);
    assert!(resources.len() <= 34);
    assert_eq!(resources.len(), stream.stats().tasks);
    assert!(stream.stats().nodes < 75);
    assert!(poll(&mut stream).is_pending());
    let polled = gate.0.borrow().polls;
    assert!(polled <= 34);
    assert!(poll(&mut stream).is_pending());
    assert_eq!(gate.0.borrow().polls, polled);
    drop(stream);
    assert!(resources.is_empty());
    assert_eq!(gate.0.borrow().drops, polled);
}

#[test]
fn html_frames_describe_hidden_nodes_and_escape_the_complete_data_block() {
    let mut stream = stream_with(Resources::default(), Config::default(), |_, _| {
        view().fill((
            text().content("visible"),
            component(|scope| {
                let hidden = group()
                    .fill(text().content("</script><script>bad()</script>"))
                    .mount(scope)?;
                hidden.suspend()?;
                scope.retain(hidden)
            }),
        ))
    })
    .unwrap();
    let shell = frame(&mut stream);
    let initial: serde_json::Value = serde_json::from_str(shell.initial.as_ref().unwrap()).unwrap();
    assert!(
        initial
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["suspended"] == true)
    );
    assert!(!shell.body().contains("bad()"));
    let bootstrap = Bootstrap {
        root_id: "app\"&".into(),
        data_id: "data".into(),
        script_url: "/stream.js".into(),
        nonce: Some("nonce\"&".into()),
    };
    let html = shell.to_html(&bootstrap).unwrap();
    assert!(
        bootstrap
            .head_script()
            .unwrap()
            .contains("nonce=\"nonce&quot;&amp;\"")
    );
    assert!(!html.contains("</script><script>bad()"));
    assert!(html.contains("\\u003c/script\\u003e"));
    assert!(html.contains("id=\"app&quot;&amp;\""));
    assert!(html.contains("nonce=\"nonce&quot;&amp;\""));
    assert_eq!(html.matches("<script ").count(), 2);
    assert!(
        shell
            .to_html(&Bootstrap {
                root_id: "same".into(),
                data_id: "same".into(),
                ..bootstrap
            })
            .is_err()
    );
}

#[test]
fn html_escaping_expansion_obeys_the_transport_frame_limit() {
    let mut stream = stream_with(
        Resources::default(),
        Config {
            chunk_bytes: 1024,
            ..Config::default()
        },
        |_, _| text().content("<".repeat(100)),
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(matches!(
        shell.to_html(&Bootstrap {
            root_id: "app".into(),
            data_id: "data".into(),
            script_url: "/stream.js".into(),
            nonce: None
        }),
        Err(Error::Limit("HTML frame bytes"))
    ));
}

#[path = "select_tests.rs"]
mod select;
