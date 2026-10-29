#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::conformance::{FlexCase, flex_cases};
use glory_render::{Change, NodeId, Patch, Query, QueryValue, Rect};
use glory_ui::App;
use std::{cell::RefCell, rc::Rc};

fn bounds(app: &App, id: NodeId) -> Rect {
    let result = Rc::new(RefCell::new(None));
    let copy = result.clone();
    app.request(Query::Bounds(id), move |v| {
        *copy.borrow_mut() = Some(v.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    let Some(QueryValue::Bounds(r)) = result.borrow_mut().take() else {
        panic!("bounds")
    };
    r
}
fn verify(app: &App, parent: NodeId, children: &[NodeId], case: &FlexCase) {
    let parent = bounds(app, parent);
    assert!((parent.width - 300.0).abs() < 0.03 && (parent.height - 200.0).abs() < 0.03);
    for (&id, expected) in children.iter().zip(case.expected) {
        let actual = bounds(app, id);
        for (value, expected) in [
            (actual.x - parent.x, expected.x),
            (actual.y - parent.y, expected.y),
            (actual.width, expected.width),
            (actual.height, expected.height),
        ] {
            assert!(
                (value - expected).abs() < 0.03,
                "{}: {actual:?}, expected={:?}, parent={parent:?}",
                case.name,
                case.expected
            );
        }
    }
}
#[test]
fn declared_flex_values_have_actual_definite_size_layout_in_both_axes() {
    let cases = flex_cases();
    assert_eq!(cases.len(), 46);
    for case in cases {
        let app = App::new(Renderer::new);
        let parent = app.scope().create(case.parent_descriptor()).unwrap();
        let scope = app.scope().child_in(parent).unwrap();
        let children: Vec<_> = (0..2)
            .map(|_| scope.create(case.child_descriptor()).unwrap())
            .collect();
        app.flush().unwrap();
        verify(&app, parent, &children, &case);
    }
}
#[test]
fn flex_style_updates_reuse_physical_nodes_across_factor_axis_and_alignment_changes() {
    let mut output = None;
    let app = App::new(|runtime| {
        let r = Renderer::new(runtime);
        output = Some(r.clone());
        r
    });
    let renderer = output.unwrap();
    let cases: Vec<_> = flex_cases().into_iter().filter(|c| !c.text).collect();
    let parent = app.scope().create(cases[0].parent_descriptor()).unwrap();
    let scope = app.scope().child_in(parent).unwrap();
    let children: Vec<_> = (0..2)
        .map(|_| scope.create(cases[0].child_descriptor()).unwrap())
        .collect();
    app.flush().unwrap();
    let physical: Vec<_> = children
        .iter()
        .map(|id| renderer.engine_id(*id, glory_html::Part::Main).unwrap())
        .collect();
    for case in cases {
        app.scope()
            .patch(
                parent,
                Patch::Style(Change::Set(Box::new(case.parent.clone()))),
            )
            .unwrap();
        for child in &children {
            scope
                .patch(
                    *child,
                    Patch::Style(Change::Set(Box::new(case.child.clone()))),
                )
                .unwrap();
        }
        app.flush().unwrap();
        verify(&app, parent, &children, &case);
        assert_eq!(
            children
                .iter()
                .map(|id| renderer.engine_id(*id, glory_html::Part::Main).unwrap())
                .collect::<Vec<_>>(),
            physical
        );
    }
}
