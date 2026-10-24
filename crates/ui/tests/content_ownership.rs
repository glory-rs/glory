use std::cell::RefCell;
use std::rc::Rc;

use glory_render::Props;
use glory_ui::*;

#[test]
fn filled_content_keeps_component_owners_and_moves_only_the_container_root() {
    let app = App::memory();
    let scopes = Rc::new(RefCell::new(Vec::new()));
    let cleaned = Rc::new(RefCell::new(Vec::new()));
    let make = |index: usize| {
        let scopes = scopes.clone();
        let cleaned = cleaned.clone();
        component(move |scope| {
            scopes.borrow_mut().push(scope.clone());
            let cleaned = cleaned.clone();
            scope.on_cleanup(move || cleaned.borrow_mut().push(index))?;
            text().content(index.to_string()).build(scope)
        })
    };
    let mounted = view()
        .fill((make(1usize), make(2)))
        .mount(app.scope())
        .unwrap();
    app.flush().unwrap();
    assert_eq!(mounted.roots().len(), 1);
    let root = mounted.roots()[0];
    let children = app.tree().node(root).unwrap().children.clone();
    assert_eq!(children.len(), 2);
    assert_ne!(scopes.borrow()[0].id(), scopes.borrow()[1].id());
    // Removing an independently retained child context leaves its sibling live.
    scopes.borrow()[0].dispose();
    app.flush().unwrap();
    assert!(app.tree().node(children[0]).is_err());
    assert!(matches!(
        app.tree()
            .node(children[1])
            .unwrap()
            .descriptor
            .view()
            .props,
        Props::Text(_)
    ));
    assert_eq!(*cleaned.borrow(), [1]);
    let other = view().mount(app.scope()).unwrap();
    mounted.place_before(Some(&other)).unwrap();
    app.flush().unwrap();
    assert_eq!(app.tree().node(root).unwrap().children, [children[1]]);
    drop(mounted);
    app.flush().unwrap();
    assert_eq!(*cleaned.borrow(), [1, 2]);
    assert!(!scopes.borrow()[1].is_alive());
    assert_eq!(app.stats().nodes, 1);
    drop(other);
    app.flush().unwrap();
    assert_eq!(app.stats().nodes, 0);
}

#[test]
fn subtree_cleanup_unlinks_local_and_foreign_listener_owners_before_dropping_captures() {
    use glory_render::{Descriptor, EventKind};

    struct Capture(std::rc::Weak<App>, Rc<RefCell<Vec<usize>>>, usize);
    impl Drop for Capture {
        fn drop(&mut self) {
            let app = self.0.upgrade().unwrap();
            // Neither the presentation nor graph borrow survives into user Drop.
            assert_eq!(app.stats().nodes, 1);
            app.runtime_stats();
            self.1.borrow_mut().push(self.2);
        }
    }
    let app = Rc::new(App::memory());
    let parent = app.scope().child().unwrap();
    let root = parent.create(Descriptor::new(Props::View)).unwrap();
    let inner = parent.child_in(root).unwrap();
    let a = inner
        .create(Descriptor::new(Props::Button(Default::default())))
        .unwrap();
    let b = inner
        .create(Descriptor::new(Props::Button(Default::default())))
        .unwrap();
    let foreign = app.scope().child().unwrap();
    let outside = foreign
        .create(Descriptor::new(Props::Button(Default::default())))
        .unwrap();
    let dropped = Rc::new(RefCell::new(Vec::new()));
    for (owner, node, index) in [(&inner, b, 1), (&foreign, a, 2), (&parent, outside, 3)] {
        let capture = Capture(Rc::downgrade(&app), dropped.clone(), index);
        owner
            .on(node, EventKind::Activate, move |_| {
                std::hint::black_box(&capture);
            })
            .unwrap();
    }
    app.flush().unwrap();
    parent.dispose();
    app.flush().unwrap();
    let mut actual = dropped.borrow().clone();
    actual.sort();
    assert_eq!(actual, [1, 2, 3]);
    assert_eq!(
        (app.stats().nodes, app.stats().owners, app.stats().handlers),
        (1, 1, 0)
    );
    assert!(foreign.is_alive());
    assert_eq!(app.tree().roots(), [outside]);
    assert!(
        !app.dispatch(outside, glory_render::Event::Activate)
            .unwrap()
    );
}
