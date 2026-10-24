//! Optional assertion-based probes for adapter tests. No UI/engine dependency;
//! these establish port semantics, not device, IME or display acceptance.
use crate::*;
mod flex;
pub use flex::{FlexCase, flex_cases};
mod collapse;
pub use collapse::verify_collapse;

/// The same behavioral matrix runs through reference, SSR and real native DOM.
/// Browser acceptance uses the public App and real DOM for the equivalent cases.
pub fn verify(mut renderer: impl Render, runtime: RuntimeId, geometry: bool) {
    let mut ids = NodeIds::new(runtime);
    let cases = [
        (Props::View, true, false, false),
        (Props::Group, false, false, false),
        (Props::Text(TextProps::default()), true, false, false),
        (Props::Button(ButtonProps::default()), true, false, false),
        (Props::Input(InputProps::default()), true, true, false),
        (Props::Check(CheckProps::default()), true, false, true),
        (Props::Image(ImageProps::default()), true, false, false),
        (Props::Link(LinkProps::default()), true, false, false),
    ];
    for (props, boxed, edits, checks) in cases {
        let id = ids.allocate();
        let mut descriptor = Descriptor::new(props.clone());
        if boxed {
            // Establish a real CSS box: empty inline spans/anchors may have no
            // engine fragment at all. This matrix tests semantic targets, not
            // each engine's empty-inline geometry availability.
            descriptor.style = style::Style::new()
                .display(style::Display::Block)
                .width(style::Length::Px(64.))
                .height(style::Length::Px(24.));
        }
        let receipt = renderer
            .apply(&[
                Mutation::Create {
                    id,
                    node: Box::new(descriptor),
                },
                Mutation::Place {
                    id,
                    parent: None,
                    before: None,
                },
            ])
            .unwrap();
        for kind in [
            EventKind::Activate,
            EventKind::FocusWithin,
            EventKind::Navigate,
            EventKind::Keyboard,
        ] {
            renderer.listen(id, kind, true).unwrap();
            renderer.listen(id, kind, false).unwrap();
        }
        for (kind, valid) in [
            (EventKind::FocusChanged, boxed),
            (EventKind::Resize, boxed),
            (EventKind::Scroll, boxed),
            (EventKind::Anchor, boxed),
            (EventKind::ValueChanged, edits),
            (EventKind::Commit, edits),
            (EventKind::Composition, edits),
            (EventKind::CheckedChanged, checks),
            (EventKind::Drag, matches!(props, Props::View)),
            (EventKind::ColorSchemeChanged, matches!(props, Props::View)),
            (EventKind::ScrollKeyboard, matches!(props, Props::View)),
        ] {
            let result = renderer.listen(id, kind, true);
            assert_eq!(result.is_ok(), valid, "{props:?}/{kind:?}: {result:?}");
            // Failed registrations must not make teardown fail.
            renderer.listen(id, kind, false).unwrap();
        }
        for (index, query) in [
            Query::Bounds(id),
            Query::Scroll(id),
            Query::ScrollMetrics(id),
            Query::Selection(id),
            Query::Focus(id),
        ]
        .into_iter()
        .enumerate()
        {
            let token = QueryToken(index as u64);
            renderer.request(query, token).unwrap();
            let mut inputs = Vec::new();
            renderer.drain(&mut inputs);
            let responses: Vec<_> = inputs
                .into_iter()
                .filter(|input| matches!(input, Input::Response { .. }))
                .collect();
            let [
                Input::Response {
                    token: actual,
                    applied,
                    result,
                },
            ] = &responses[..]
            else {
                panic!("one accepted request must complete once: {responses:?}");
            };
            assert_eq!(*actual, token);
            assert_eq!(*applied, receipt.applied);
            let capabilities = renderer.capabilities();
            let expected = match query {
                Query::Focus(_) => capabilities.focus && boxed,
                Query::Selection(_) => capabilities.input && edits,
                Query::Bounds(_) | Query::Scroll(_) | Query::ScrollMetrics(_) => geometry && boxed,
            };
            assert_eq!(result.is_ok(), expected, "{props:?}/{query:?}: {result:?}");
            if let Ok(value) = result {
                query.validate_response(value).unwrap();
            } else {
                assert!(matches!(result, Err(Error::Unsupported(_))));
            }
        }
        assert!(matches!(
            renderer.action(Action::ScrollTo {
                node: id,
                x: f64::NAN,
                y: 0.
            }),
            Err(Error::InvalidProps(_))
        ));
        renderer.apply(&[Mutation::Dispose { id }]).unwrap();
        assert_eq!(
            renderer.listen(id, EventKind::Resize, true),
            Err(Error::NodeGone(id))
        );
        assert_eq!(
            renderer.request(Query::Bounds(id), QueryToken(99)),
            Err(Error::NodeGone(id))
        );
        assert_eq!(renderer.action(Action::Focus(id)), Err(Error::NodeGone(id)));
    }
    // Construction units preserve the adapter's accepted-prefix watermark even
    // when a later identity rejects the entire next unit.
    let plan = Plan::new(vec![
        PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::View),
        },
        PlanNode {
            parent: Some(0),
            descriptor: Descriptor::new(Props::Text(TextProps {
                content: "plan".into(),
                ..Default::default()
            })),
        },
    ])
    .unwrap();
    let first = vec![ids.allocate(), ids.allocate()];
    let prefix = renderer
        .apply(&[
            Mutation::CreatePlan {
                plan: plan.clone(),
                ids: first.clone(),
            },
            Mutation::Place {
                id: first[0],
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    let unused = ids.allocate();
    assert!(
        renderer
            .apply(&[Mutation::CreatePlan {
                plan,
                ids: vec![unused, foreign]
            }])
            .is_err()
    );
    assert_eq!(renderer.apply(&[]).unwrap(), prefix);
    assert_eq!(
        renderer.listen(unused, EventKind::Activate, true),
        Err(Error::NodeGone(unused))
    );
    renderer
        .request(Query::Bounds(first[1]), QueryToken(102))
        .unwrap();
    let mut responses = Vec::new();
    renderer.drain(&mut responses);
    assert!(responses.iter().any(|input| matches!(input, Input::Response { token: QueryToken(102), applied, .. } if *applied == prefix.applied)));
    renderer
        .apply(&[Mutation::Dispose { id: first[0] }])
        .unwrap();
    assert_eq!(
        renderer.listen(first[1], EventKind::Activate, true),
        Err(Error::NodeGone(first[1]))
    );
    assert_eq!(
        renderer.listen(foreign, EventKind::Activate, true),
        Err(Error::ForeignRuntime)
    );
    assert_eq!(
        renderer.request(Query::Bounds(foreign), QueryToken(99)),
        Err(Error::ForeignRuntime)
    );
    assert_eq!(
        renderer.action(Action::Focus(foreign)),
        Err(Error::ForeignRuntime)
    );
}
