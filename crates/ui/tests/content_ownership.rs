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
        app.tree().node(children[1]).unwrap().descriptor.props,
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
