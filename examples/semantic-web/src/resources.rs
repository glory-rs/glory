//! Controlled asynchronous I/O for browser lifecycle acceptance. The test can
//! resolve a pending key; dropped rows remove their jobs instead of retaining
//! a history of visited data. No timers or synthetic synchronous executor.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};

use glory::prelude::*;
use glory::web::WebApp;
use wasm_bindgen::prelude::*;

type Result<T = ()> = std::result::Result<T, JsValue>;
struct Job {
    key: usize,
    result: Option<std::result::Result<String, String>>,
    waker: Option<Waker>,
}
#[derive(Default)]
struct Monitor {
    next: usize,
    started: usize,
    finished: usize,
    cancelled: usize,
    peak: usize,
    jobs: BTreeMap<usize, Weak<RefCell<Job>>>,
}
struct Request {
    id: usize,
    job: Rc<RefCell<Job>>,
    monitor: Rc<RefCell<Monitor>>,
    finished: bool,
}
impl Request {
    fn new(key: usize, monitor: Rc<RefCell<Monitor>>) -> Self {
        let job = Rc::new(RefCell::new(Job {
            key,
            result: None,
            waker: None,
        }));
        let id = {
            let mut monitor = monitor.borrow_mut();
            let id = monitor.next;
            monitor.next += 1;
            monitor.started += 1;
            monitor.jobs.insert(id, Rc::downgrade(&job));
            monitor.peak = monitor.peak.max(monitor.jobs.len());
            id
        };
        Self {
            id,
            job,
            monitor,
            finished: false,
        }
    }
}
impl Future for Request {
    type Output = std::result::Result<String, String>;
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let result = self.job.borrow_mut().result.take();
        match result {
            Some(result) => {
                self.finished = true;
                Poll::Ready(result)
            }
            None => {
                self.job.borrow_mut().waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        let mut monitor = self.monitor.borrow_mut();
        monitor.jobs.remove(&self.id);
        if self.finished {
            monitor.finished += 1;
        } else {
            monitor.cancelled += 1;
        }
    }
}
struct Demo {
    web: WebApp,
    source: Cage<usize>,
    position: Cage<f64>,
    resource: Resource<usize, String, String>,
}
thread_local! {
    static APP: RefCell<Option<Demo>> = const { RefCell::new(None) };
    static MONITOR: RefCell<Option<Rc<RefCell<Monitor>>>> = const { RefCell::new(None) };
    static REGISTRY: RefCell<Option<Resources>> = const { RefCell::new(None) };
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
pub(crate) fn clear() {
    APP.with(|app| {
        let previous = app.borrow_mut().take();
        drop(previous);
    });
}
fn label(state: &ResourceState<usize, String, String>) -> String {
    match state {
        ResourceState::Idle => "idle".into(),
        ResourceState::Pending { key } => format!("pending:{key}"),
        ResourceState::Ready { value, .. } => value.to_string(),
        ResourceState::Failed { error, .. } => format!("failed:{error}"),
        ResourceState::Unavailable { error, .. } => format!("unavailable:{error}"),
    }
}
#[wasm_bindgen]
pub fn resource_mount() -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("app"))
        .ok_or_else(|| js("app root missing"))?;
    let monitor = Rc::new(RefCell::new(Monitor::default()));
    let jobs = monitor.clone();
    let registry = Resources::default();
    let registered = registry.clone();
    let mut state = None;
    let web = glory::web::launch_with(root, |scope| {
        let source = scope.cage(7);
        let position = scope.cage(0.0);
        let loader = jobs.clone();
        let resource = scope.owner().resource_seeded(
            move || source.get(),
            move |key| Request::new(key, loader.clone()),
            Some(ResourceSeed {
                key: 7,
                result: Ok("seed:7".into()),
            }),
        );
        state = Some((source, position, resource));
        view().class("resources").fill((
            text().class("resource-main").content(resource.map(label)),
            list(
                100_000_000_usize,
                |index| index + 10_000,
                move |_, key| {
                    let jobs = jobs.clone();
                    let registered = registered.clone();
                    widget(move |scope| {
                        let jobs = jobs.clone();
                        let resource = registered
                            .bind(
                                scope.owner(),
                                &format!("row/{key}"),
                                "row/1",
                                move || key,
                                move |key| Request::new(key, jobs.clone()),
                            )
                            .expect("unique row resource");
                        text()
                            .class("resource-row")
                            .content(resource.map(label))
                            .show_in(scope)?;
                        Ok(())
                    })
                },
            )
            .height(240.0)
            .row_height(24.0)
            .overscan(2)
            .position(position),
        ))
    })
    .map_err(js)?;
    let (source, position, resource) = state.expect("fixture state");
    APP.with(|app| {
        *app.borrow_mut() = Some(Demo {
            web,
            source,
            position,
            resource,
        })
    });
    MONITOR.with(|slot| *slot.borrow_mut() = Some(monitor));
    REGISTRY.with(|slot| *slot.borrow_mut() = Some(registry));
    Ok(())
}
fn update(f: impl FnOnce(&Demo) -> Result) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let app = app.as_ref().ok_or_else(|| js("resource app missing"))?;
        f(app)?;
        app.web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn resource_key(key: usize) -> Result {
    update(|app| {
        app.source.revise(|value| *value = key);
        Ok(())
    })
}
#[wasm_bindgen]
pub fn resource_refresh() -> Result {
    update(|app| app.resource.refresh().map_err(js))
}
#[wasm_bindgen]
pub fn resource_cancel() -> Result {
    update(|app| app.resource.cancel().map_err(js))
}
#[wasm_bindgen]
pub fn resource_seek(offset: f64) -> Result {
    update(|app| {
        app.position.revise(|value| *value = offset);
        Ok(())
    })
}
#[wasm_bindgen]
pub fn resource_finish(key: usize, fail: bool) -> usize {
    let jobs = MONITOR
        .with(|slot| {
            slot.borrow().as_ref().map(|monitor| {
                monitor
                    .borrow()
                    .jobs
                    .values()
                    .filter_map(Weak::upgrade)
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    let mut completed = 0;
    for job in jobs {
        let waker = {
            let mut job = job.borrow_mut();
            if job.key != key {
                continue;
            }
            job.result = Some(if fail {
                Err(format!("error:{key}"))
            } else {
                Ok(format!("loaded:{key}"))
            });
            completed += 1;
            job.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    completed
}
#[wasm_bindgen]
pub fn resource_stats() -> String {
    let registrations =
        REGISTRY.with(|registry| registry.borrow().as_ref().map_or(0, Resources::len));
    let (started, finished, cancelled, pending, peak) = MONITOR.with(|slot| {
        let slot = slot.borrow();
        let Some(monitor) = slot.as_ref() else {
            return (0, 0, 0, 0, 0);
        };
        let monitor = monitor.borrow();
        (
            monitor.started,
            monitor.finished,
            monitor.cancelled,
            monitor.jobs.len(),
            monitor.peak,
        )
    });
    let (tasks, graph, slots, nodes) = APP.with(|app| {
        let app = app.borrow();
        let Some(app) = app.as_ref() else {
            return (0, 0, 0, 0);
        };
        let stats = app.web.app().runtime_stats();
        (
            stats.tasks,
            stats.nodes,
            stats.node_slots,
            app.web.app().stats().nodes,
        )
    });
    format!(
        "{{\"started\":{started},\"finished\":{finished},\"cancelled\":{cancelled},\"pending\":{pending},\"peak\":{peak},\"tasks\":{tasks},\"graph\":{graph},\"slots\":{slots},\"nodes\":{nodes},\"registrations\":{registrations}}}"
    )
}
