use glory_core::Runtime;
use glory_routing::*;
use std::{cell::Cell, rc::Rc};
fn location(value: &str) -> Location {
    Location::parse(value).unwrap()
}
fn routes() -> Routes {
    Routes::builder()
        .root(RouteDef::new(RouteKey::<Params>::new("page"), "*path"))
        .finish()
        .unwrap()
}
#[test]
fn memory_history_truncates_forward_preserves_replace_identity_and_bounds_storage() {
    let history = MemoryHistory::with_capacity(location("/"), 3);
    history.push(&location("/a")).unwrap();
    let first = history.current().unwrap();
    history.push(&location("/a")).unwrap();
    assert_ne!(history.current().unwrap().key, first.key);
    history.traverse(-1).unwrap();
    history.push(&location("/b")).unwrap();
    assert_eq!(history.preview(1).unwrap(), Traversal::Boundary);
    let key = history.current().unwrap().key;
    history.replace(&location("/c")).unwrap();
    assert_eq!(history.current().unwrap().key, key);
    history.push(&location("/d")).unwrap();
    assert_eq!(history.len(), 3);
}

#[test]
fn native_deep_links_require_an_explicit_scheme_and_host_and_keep_query_and_hash() {
    let runtime = Runtime::new();
    let scope = runtime.scope();
    let router = Router::new(&scope, routes(), Rc::new(MemoryHistory::new(location("/")))).unwrap();
    let handle = router.handle();
    let mut native = glory_routing::native::NativeNavigation::new(handle.clone());
    assert!(
        native
            .deep_link("app://other/page/1", "app", "owned")
            .is_err()
    );
    assert_eq!(handle.current().unwrap().location().path(), "/");
    native
        .deep_link("app://owned/page/1?tab=a#detail", "app", "owned")
        .unwrap();
    assert_eq!(
        handle.current().unwrap().location(),
        &location("/page/1?tab=a#detail")
    );
}

#[test]
fn internal_navigation_cannot_leave_the_mount_and_active_links_use_segment_boundaries() {
    let runtime = Runtime::new();
    let scope = runtime.scope();
    let definitions = Routes::builder()
        .base("/app")
        .root(RouteDef::new(RouteKey::<Params>::new("page"), "*path"))
        .finish()
        .unwrap();
    let router = Router::new(
        &scope,
        definitions,
        Rc::new(MemoryHistory::new(location("/app/users/1"))),
    )
    .unwrap();
    let handle = router.handle();
    assert_eq!(handle.push(location("/apple")), Err(Error::OutsideBase));
    assert!(
        handle
            .is_active_path(&location("/app/users"), true)
            .unwrap()
    );
    assert!(!handle.is_active_path(&location("/app/user"), true).unwrap());
    assert_eq!(handle.current().unwrap().location().path(), "/app/users/1");
}
#[test]
fn native_interactive_back_can_cancel_commit_and_reject_stale_gestures() {
    let runtime = Runtime::new();
    let scope = runtime.scope();
    let history = Rc::new(MemoryHistory::new(location("/")));
    let router = Router::new(&scope, routes(), history.clone()).unwrap();
    let handle = router.handle();
    handle.push(location("/a")).unwrap();
    drop(handle.prepare_back().unwrap());
    assert_eq!(history.current().unwrap().location, location("/a"));
    handle.prepare_back().unwrap().unwrap().commit().unwrap();
    assert_eq!(history.current().unwrap().location, location("/"));
    handle.forward().unwrap();
    let stale = handle.prepare_back().unwrap().unwrap();
    handle.push(location("/b")).unwrap();
    assert_eq!(stale.commit(), Err(Error::StaleNavigation));
    assert_eq!(handle.current().unwrap().location(), &location("/b"));
    assert_eq!(handle.push(location("/b")).unwrap(), Navigation::Noop);
}
#[test]
fn blocked_guards_reentrancy_redirect_cycles_and_owner_retirement_are_explicit() {
    let runtime = Runtime::new();
    let scope = runtime.scope();
    let history = Rc::new(MemoryHistory::new(location("/")));
    let router = Router::new(&scope, routes(), history.clone()).unwrap();
    let handle = router.handle();
    router.guard(|ctx| match ctx.location.path() {
        "/blocked" => Ok(Decision::Block),
        "/old" => Ok(Decision::Redirect(location("/new"))),
        "/loop" => Ok(Decision::Redirect(location("/loop"))),
        "/reentrant" => {
            assert_eq!(ctx.router.push(location("/other")), Err(Error::Busy));
            Ok(Decision::Allow)
        }
        _ => Ok(Decision::Allow),
    });
    assert_eq!(handle.push(location("/blocked")), Err(Error::Blocked));
    assert_eq!(history.len(), 1);
    assert_eq!(handle.push(location("/loop")), Err(Error::RedirectLoop));
    handle.push(location("/old")).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history.current().unwrap().location, location("/new"));
    handle.push(location("/reentrant")).unwrap();
    drop(router);
    assert!(handle.is_alive());
    scope.owner().dispose();
    assert_eq!(handle.push(location("/late")), Err(Error::Gone));
}
#[test]
fn same_url_entries_have_independent_bounded_scroll_positions() {
    let history = MemoryHistory::new(location("/"));
    let first = history.current().unwrap().key.unwrap();
    let second = history.push(&location("/")).unwrap().key.unwrap();
    let mut scroll = ScrollMemory::new(2);
    scroll.record(first.clone(), "main", 0.0, 10.0).unwrap();
    scroll.record(second.clone(), "main", 0.0, 40.0).unwrap();
    assert_eq!(scroll.restore(&first, "main"), Some((0.0, 10.0)));
    assert_eq!(scroll.restore(&second, "main"), Some((0.0, 40.0)));
    assert!(scroll.record(first, "main", 0.0, f64::NAN).is_err());
    scroll
        .record(EntryKey("third".into()), "main", 0.0, 3.0)
        .unwrap();
    assert_eq!(scroll.len(), 2);
}
#[test]
fn host_failure_does_not_publish_and_distinct_routers_do_not_share_state() {
    struct Reject(MemoryHistory);
    impl History for Reject {
        fn current(&self) -> Result<Entry> {
            self.0.current()
        }
        fn push(&self, _: &Location) -> Result<Entry> {
            Err(Error::History("refused".into()))
        }
        fn replace(&self, _: &Location) -> Result<Entry> {
            Err(Error::History("refused".into()))
        }
        fn preview(&self, _: i32) -> Result<Traversal> {
            Ok(Traversal::Boundary)
        }
        fn traverse(&self, _: i32) -> Result<Option<Entry>> {
            Ok(None)
        }
    }
    let runtime = Runtime::new();
    let scope = runtime.scope();
    let a = Router::new(
        &scope,
        routes(),
        Rc::new(Reject(MemoryHistory::new(location("/")))),
    )
    .unwrap();
    assert!(a.handle().push(location("/failed")).is_err());
    assert_eq!(a.handle().current().unwrap().location(), &location("/"));
    let b = Router::new(&scope, routes(), Rc::new(MemoryHistory::new(location("/")))).unwrap();
    b.handle().push(location("/b")).unwrap();
    assert_eq!(a.handle().current().unwrap().location(), &location("/"));
}
#[test]
fn superseding_async_navigation_wakes_and_disposes_guard_owned_work() {
    use std::{
        future::Future,
        pin::pin,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };
    struct Signal(AtomicUsize);
    impl Wake for Signal {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let runtime = Runtime::new();
    let scope = runtime.scope();
    let router = Router::new(&scope, routes(), Rc::new(MemoryHistory::new(location("/")))).unwrap();
    let source = scope.cage(1u32);
    let captured = Rc::new(Cell::new(None::<glory_core::Bond<u32>>));
    let saved = captured.clone();
    router.async_guard(move |ctx| {
        if ctx.location.path() == "/slow" {
            let saved = saved.clone();
            Box::pin(async move {
                saved.set(Some(source.map(|value| value + 1)));
                std::future::pending::<Result<Decision>>().await
            })
        } else {
            Box::pin(async { Ok(Decision::Allow) })
        }
    });
    let handle = router.handle();
    let mut slow = pin!(handle.push_async(location("/slow")));
    let signal = Arc::new(Signal(AtomicUsize::new(0)));
    let waker = Waker::from(signal.clone());
    let mut cx = Context::from_waker(&waker);
    assert!(slow.as_mut().poll(&mut cx).is_pending());
    let mut fast = pin!(handle.push_async(location("/fast")));
    assert_eq!(
        fast.as_mut().poll(&mut cx),
        Poll::Ready(Ok(Navigation::Committed))
    );
    assert!(!captured.get().unwrap().is_alive());
    assert!(signal.0.load(Ordering::SeqCst) > 0);
    assert_eq!(
        slow.as_mut().poll(&mut cx),
        Poll::Ready(Err(Error::Superseded))
    );
    assert_eq!(handle.current().unwrap().location(), &location("/fast"));
}
