use glory_render::{Error as RenderError, NodeId, Query};
use glory_ui::{App, view};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

fn query_chain(app: Weak<App>, node: NodeId, index: usize, seen: Rc<RefCell<Vec<usize>>>) {
    let live = app.upgrade().unwrap();
    live.request(Query::Bounds(node), move |result| {
        assert!(matches!(result, Err(RenderError::Unsupported(_))));
        seen.borrow_mut().push(index);
        if index < 4 {
            query_chain(app, node, index + 1, seen);
        }
    })
    .unwrap();
}

#[test]
fn local_query_callbacks_yield_at_budget_keep_order_and_finish_without_reentry() {
    let app = Rc::new(App::memory());
    app.mount(view()).unwrap();
    let node = app.tree().roots()[0];
    let seen = Rc::new(RefCell::new(Vec::new()));
    query_chain(Rc::downgrade(&app), node, 0, seen.clone());
    for limit in [0, App::MAX_SETTLE_PASSES + 1] {
        assert!(matches!(
            app.settle(limit),
            Err(glory_ui::Error::SettleLimit { .. })
        ));
    }
    assert!(seen.borrow().is_empty());
    let first = app.settle(2).unwrap();
    assert_eq!(first.passes, 2);
    assert!(first.pending);
    assert_eq!(&*seen.borrow(), &[0, 1]);
    let last = app.settle(8).unwrap();
    assert_eq!(last.passes, 3);
    assert!(!last.pending);
    assert_eq!(&*seen.borrow(), &[0, 1, 2, 3, 4]);
    assert_eq!(app.stats().queries, 0);
    assert_eq!(app.settle(8).unwrap().passes, 1);
    app.clear().unwrap();
    assert!(!app.settle(1).unwrap().pending);
}
