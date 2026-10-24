use glory_render::*;
use glory_ui::{App, widget};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn raw_pointer_captures_within_only_groups_and_stops_at_common_ancestry() {
    let app = App::memory();
    let group = app.scope().cage(None);
    let first = app.scope().cage(None);
    let second = app.scope().cage(None);
    let values = Rc::new(RefCell::new(Vec::new()));
    let seen = values.clone();
    app.mount(widget(move |scope| {
        let root = scope.create(Descriptor::new(Props::Group))?;
        group.revise(|id| *id = Some(root));
        let children = scope.child_in(root)?;
        first.revise(|id| {
            *id = Some(
                children
                    .create(Descriptor::new(Props::Button(ButtonProps::default())))
                    .unwrap(),
            )
        });
        second.revise(|id| *id = Some(children.create(Descriptor::new(Props::View)).unwrap()));
        let seen = seen.clone();
        scope.on(root, EventKind::PointerWithin, move |event| {
            seen.borrow_mut().push(event)
        })?;
        scope.retain(children)?;
        Ok(())
    }))
    .unwrap();
    let a = first.get().unwrap();
    let b = second.get().unwrap();
    let root = group.get().unwrap();
    app.dispatch(
        a,
        Event::Pointer {
            from: a,
            related: None,
            phase: PointerPhase::Enter,
        },
    )
    .unwrap();
    assert_eq!(
        *values.borrow(),
        [Event::PointerWithin {
            from: a,
            related: None,
            phase: PointerPhase::Enter
        }]
    );
    app.dispatch(
        a,
        Event::Pointer {
            from: a,
            related: Some(b),
            phase: PointerPhase::Leave,
        },
    )
    .unwrap();
    app.dispatch(
        b,
        Event::Pointer {
            from: b,
            related: Some(a),
            phase: PointerPhase::Enter,
        },
    )
    .unwrap();
    assert_eq!(
        values.borrow().len(),
        1,
        "a sibling transition stays inside the logical Group"
    );
    app.dispatch(
        root,
        Event::PointerWithin {
            from: b,
            related: None,
            phase: PointerPhase::Leave,
        },
    )
    .unwrap();
    assert_eq!(values.borrow().len(), 2);
}

#[test]
fn pointer_within_rechecks_origin_after_direct_callback_retirement() {
    let app = App::memory();
    let calls = Rc::new(RefCell::new(0usize));
    let seen = calls.clone();
    let parent = app.scope().create(Descriptor::new(Props::View)).unwrap();
    app.scope()
        .on(parent, EventKind::PointerWithin, move |_| {
            *seen.borrow_mut() += 1
        })
        .unwrap();
    let owner = app.scope().child_in(parent).unwrap();
    let source = owner.create(Descriptor::new(Props::View)).unwrap();
    let weak = owner.downgrade();
    owner
        .on(source, EventKind::Pointer, move |_| {
            if let Some(owner) = weak.upgrade() {
                owner.dispose();
            }
        })
        .unwrap();
    app.flush().unwrap();
    assert!(
        app.dispatch(
            source,
            Event::Pointer {
                from: source,
                related: None,
                phase: PointerPhase::Enter
            }
        )
        .unwrap()
    );
    assert_eq!(*calls.borrow(), 0);
    assert!(app.tree().node(source).is_err());
    assert!(
        !app.dispatch(
            parent,
            Event::PointerWithin {
                from: source,
                related: None,
                phase: PointerPhase::Leave
            }
        )
        .unwrap()
    );
}
