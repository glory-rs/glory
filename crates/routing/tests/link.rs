#![cfg(all(feature = "widgets", not(target_arch = "wasm32")))]

use glory_core::{AccessError, executor::Executor};
use glory_render::{Event, memory::Memory};
use glory_routing::*;
use glory_ui::{App, Widget};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

fn location(path: &str) -> Location {
    Location::parse(path).unwrap()
}
fn router(app: &App) -> Router {
    let routes = Routes::builder()
        .root(RouteDef::new(RouteKey::<Params>::new("page"), "*path"))
        .finish()
        .unwrap();
    Router::new(
        app.scope(),
        routes,
        Rc::new(MemoryHistory::new(location("/"))),
    )
    .unwrap()
}
struct Pending(Rc<Cell<usize>>);
impl Future for Pending {
    type Output = Result<Decision>;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn scoped_link_runs_sync_guards_without_requiring_a_task_host() {
    let app = App::memory();
    let router = router(&app);
    let replies = Rc::new(RefCell::new(Vec::new()));
    let out = replies.clone();
    let mounted = link_with(
        app.scope(),
        router.handle(),
        location("/next"),
        move |result| {
            out.borrow_mut().push(result);
        },
    )
    .unwrap()
    .label("Next")
    .class("route-link")
    .mount(app.scope())
    .unwrap();
    app.dispatch(mounted.roots()[0], Event::Activate).unwrap();
    assert_eq!(&*replies.borrow(), &[Ok(Navigation::Committed)]);
    assert_eq!(
        router.handle().current().unwrap().location(),
        &location("/next")
    );
    assert_eq!(app.runtime_stats().tasks, 0);
}

#[test]
fn async_link_uses_the_explicit_host_and_retires_reply_allocations_with_its_handler() {
    let executor = Rc::new(Executor::new(8));
    let app = App::with_host(executor.clone(), Memory::new);
    let router = router(&app);
    let guards = Rc::new(Cell::new(0));
    let calls = guards.clone();
    router.async_guard(move |ctx| {
        calls.set(calls.get() + 1);
        Box::pin(async move {
            Ok(if ctx.location.path() == "/next" {
                Decision::Redirect(location("/final"))
            } else {
                Decision::Allow
            })
        })
    });
    let source = app.scope().cage(1u32);
    let mapped = Rc::new(Cell::new(None));
    let saved = mapped.clone();
    let replies = Rc::new(RefCell::new(Vec::new()));
    let out = replies.clone();
    let mounted = link_with(
        app.scope(),
        router.handle(),
        location("/next"),
        move |result| {
            saved.set(Some(source.map(|value| value + 1)));
            out.borrow_mut().push(result);
        },
    )
    .unwrap()
    .mount(app.scope())
    .unwrap();
    app.dispatch(mounted.roots()[0], Event::Activate).unwrap();
    assert_eq!(guards.get(), 0);
    assert!(replies.borrow().is_empty());
    assert_eq!(executor.len(), 1);
    assert!(executor.poll_one());
    assert_eq!(guards.get(), 2);
    assert_eq!(&*replies.borrow(), &[Ok(Navigation::Committed)]);
    assert_eq!(
        router.handle().current().unwrap().location(),
        &location("/final")
    );
    assert_eq!(mapped.get().unwrap().get(), 2);
    assert!(executor.is_empty());
    drop(mounted);
    app.flush().unwrap();
    assert!(!mapped.get().unwrap().is_alive());
    assert!(source.is_alive());
}

#[test]
fn missing_or_limited_hosts_report_execution_errors_before_running_guards() {
    for limited in [false, true] {
        let app = if limited {
            App::with_host(Rc::new(Executor::new(0)), Memory::new)
        } else {
            App::memory()
        };
        let router = router(&app);
        let guards = Rc::new(Cell::new(0));
        let calls = guards.clone();
        router.async_guard(move |_| {
            calls.set(calls.get() + 1);
            Box::pin(async { Ok(Decision::Allow) })
        });
        let result = Rc::new(RefCell::new(None));
        let out = result.clone();
        let mounted = link_with(
            app.scope(),
            router.handle(),
            location("/next"),
            move |result| {
                *out.borrow_mut() = Some(result);
            },
        )
        .unwrap()
        .mount(app.scope())
        .unwrap();
        app.dispatch(mounted.roots()[0], Event::Activate).unwrap();
        assert!(
            matches!(&*result.borrow(), Some(Err(Error::Execution(error)))
            if if limited { matches!(error, AccessError::Host(_)) } else { *error == AccessError::NoHost })
        );
        assert_eq!(guards.get(), 0);
        assert_eq!(
            router.handle().current().unwrap().location(),
            &location("/")
        );
        assert_eq!(app.runtime_stats().tasks, 0);
    }
}

#[test]
fn handler_drop_and_explicit_owner_retirement_cancel_pending_navigation_without_reply() {
    for (retire_owner, polled) in [(false, false), (false, true), (true, false), (true, true)] {
        let executor = Rc::new(Executor::new(8));
        let app = App::with_host(executor.clone(), Memory::new);
        let router = router(&app);
        let drops = Rc::new(Cell::new(0));
        let dropped = drops.clone();
        router.async_guard(move |_| Box::pin(Pending(dropped.clone())));
        let replies = Rc::new(Cell::new(0));
        let calls = replies.clone();
        let owner = app.scope().child().unwrap();
        let mounted = link_with(&owner, router.handle(), location("/next"), move |_| {
            calls.set(calls.get() + 1);
        })
        .unwrap()
        .mount(app.scope())
        .unwrap();
        app.dispatch(mounted.roots()[0], Event::Activate).unwrap();
        if polled {
            assert!(executor.poll_one());
        }
        assert_eq!(drops.get(), 0);
        if retire_owner {
            owner.dispose();
        } else {
            drop(mounted);
        }
        app.flush().unwrap();
        assert!(executor.is_empty());
        assert_eq!(app.runtime_stats().tasks, 0);
        assert_eq!(drops.get(), usize::from(polled));
        assert_eq!(replies.get(), 0);
        assert_eq!(
            router.handle().current().unwrap().location(),
            &location("/")
        );
    }
}

#[test]
fn async_link_reports_guard_refusals_without_committing_history() {
    for (decision, expected) in [
        (Decision::Block, Error::Blocked),
        (Decision::Denied(401), Error::Denied(401)),
    ] {
        let executor = Rc::new(Executor::new(8));
        let app = App::with_host(executor.clone(), Memory::new);
        let router = router(&app);
        router.async_guard(move |_| {
            let decision = decision.clone();
            Box::pin(async move { Ok(decision) })
        });
        let result = Rc::new(RefCell::new(None));
        let out = result.clone();
        let mounted = link_with(
            app.scope(),
            router.handle(),
            location("/next"),
            move |result| {
                *out.borrow_mut() = Some(result);
            },
        )
        .unwrap()
        .mount(app.scope())
        .unwrap();
        app.dispatch(mounted.roots()[0], Event::Activate).unwrap();
        assert!(executor.poll_one());
        assert_eq!(&*result.borrow(), &Some(Err(expected)));
        assert_eq!(
            router.handle().current().unwrap().location(),
            &location("/")
        );
        assert!(executor.is_empty());
    }
}

#[test]
fn guards_that_retire_the_initiating_owner_cannot_commit_or_reply_from_an_active_task() {
    for asynchronous in [false, true] {
        let executor = Rc::new(Executor::new(8));
        let app = App::with_host(executor.clone(), Memory::new);
        let router = router(&app);
        let owner = app.scope().child().unwrap();
        let retire = owner.clone();
        if asynchronous {
            router.async_guard(move |_| {
                let retire = retire.clone();
                Box::pin(async move {
                    retire.dispose();
                    Ok(Decision::Allow)
                })
            });
        } else {
            router.guard(move |_| {
                retire.dispose();
                Ok(Decision::Allow)
            });
        }
        let replies = Rc::new(Cell::new(0));
        let calls = replies.clone();
        let mounted = link_with(&owner, router.handle(), location("/next"), move |_| {
            calls.set(calls.get() + 1);
        })
        .unwrap()
        .mount(app.scope())
        .unwrap();
        app.dispatch(mounted.roots()[0], Event::Activate).unwrap();
        if asynchronous {
            assert!(executor.poll_one());
        }
        assert_eq!(
            router.handle().current().unwrap().location(),
            &location("/")
        );
        assert_eq!(replies.get(), 0);
        assert_eq!(app.runtime_stats().tasks, 0);
        assert!(executor.is_empty());
    }
}

#[test]
fn repeated_async_activations_supersede_old_guards_and_report_each_live_result_once() {
    let executor = Rc::new(Executor::new(8));
    let app = App::with_host(executor.clone(), Memory::new);
    let router = router(&app);
    let guards = Rc::new(Cell::new(0));
    let calls = guards.clone();
    let drops = Rc::new(Cell::new(0));
    let dropped = drops.clone();
    router.async_guard(move |_| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            Box::pin(Pending(dropped.clone()))
        } else {
            Box::pin(async { Ok(Decision::Allow) })
        }
    });
    let replies = Rc::new(RefCell::new(Vec::new()));
    let out = replies.clone();
    let mounted = link_with(
        app.scope(),
        router.handle(),
        location("/next"),
        move |result| {
            out.borrow_mut().push(result);
        },
    )
    .unwrap()
    .mount(app.scope())
    .unwrap();
    for _ in 0..2 {
        app.dispatch(mounted.roots()[0], Event::Activate).unwrap();
    }
    for _ in 0..8 {
        if !executor.poll_one() {
            break;
        }
    }
    assert!(executor.is_empty());
    assert_eq!(guards.get(), 2);
    assert_eq!(drops.get(), 1);
    assert_eq!(
        &*replies.borrow(),
        &[Ok(Navigation::Committed), Err(Error::Superseded)]
    );
    assert_eq!(
        router.handle().current().unwrap().location(),
        &location("/next")
    );
    assert_eq!(app.runtime_stats().tasks, 0);
}
