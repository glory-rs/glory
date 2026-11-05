#![cfg(not(target_arch = "wasm32"))]
use glory_render::*;
fn node(open: bool, revision: u64, effect: PresenceEffect) -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.semantics = Some(Box::new(Semantics {
        hidden: !open,
        inert: !open,
        presence: Some(Box::new(PresenceConfig {
            open,
            effect,
            duration_millis: 200,
            distance: 16,
            revision,
            ..Default::default()
        })),
        ..Default::default()
    }));
    d
}
#[test]
fn native_preflight_and_direct_rejection_preserve_successful_prefix_and_query_watermark() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let root = ids.allocate();
    let bad = ids.allocate();
    let mut r = glory_apace::Renderer::new(runtime);
    r.apply(&[
        Mutation::Create {
            id: root,
            node: Box::new(node(true, 0, PresenceEffect::Fade)),
        },
        Mutation::Place {
            id: root,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    r.request(Query::Bounds(root), QueryToken(1)).unwrap();
    let receipt = r.apply(&[]).unwrap();
    for effect in [
        PresenceEffect::Scale,
        PresenceEffect::ScaleX,
        PresenceEffect::ScaleY,
        PresenceEffect::SlideUp,
        PresenceEffect::SlideDown,
        PresenceEffect::SlideLeft,
        PresenceEffect::SlideRight,
    ] {
        let d = node(true, 0, effect);
        assert!(matches!(
            r.preflight(d.view(), None),
            Err(Error::Unsupported(_))
        ));
    }
    assert!(
        r.apply(&[
            Mutation::Patch {
                id: root,
                patch: Patch::Semantics(Change::Set(
                    node(false, 1, PresenceEffect::Fade).semantics
                ))
            },
            Mutation::Create {
                id: bad,
                node: Box::new(node(true, 0, PresenceEffect::Scale))
            }
        ])
        .is_err()
    );
    assert_eq!(r.stats().nodes, 1);
    assert!(r.engine_id(bad, glory_html::Part::Main).is_err());
    let mut inputs = Vec::new();
    r.drain(&mut inputs);
    assert!(
        inputs
            .iter()
            .any(|i| matches!(i,Input::Response{applied,..}if *applied==receipt.applied))
    );
}
#[test]
fn suspended_and_zero_viewport_settle_once_without_replay_or_retained_disposal() {
    for zero in [false, true] {
        let runtime = RuntimeId::new();
        let id = NodeIds::new(runtime).allocate();
        let mut r = glory_apace::Renderer::with_config(
            runtime,
            glory_apace::Config {
                motion: MotionPreference::NoPreference,
                ..Default::default()
            },
        );
        r.set_viewport(160, 160, 1.).unwrap();
        r.apply(&[
            Mutation::Create {
                id,
                node: Box::new(node(true, 0, PresenceEffect::Fade)),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
        r.listen(id, EventKind::PresenceComplete, true).unwrap();
        let mut inputs = Vec::new();
        r.drain(&mut inputs);
        inputs.clear();
        let physical = r.engine_id(id, glory_html::Part::Main).unwrap();
        r.apply(&[Mutation::Patch {
            id,
            patch: Patch::Semantics(Change::Set(node(false, 1, PresenceEffect::Fade).semantics)),
        }])
        .unwrap();
        if zero {
            r.set_viewport(0, 0, 1.).unwrap();
        } else {
            r.apply(&[Mutation::Suspend { id }]).unwrap();
        }
        r.drain(&mut inputs);
        assert_eq!(
            inputs
                .iter()
                .filter(|i| matches!(
                    i,
                    Input::Event {
                        event: Event::PresenceComplete { revision: 1 },
                        ..
                    }
                ))
                .count(),
            1
        );
        inputs.clear();
        if zero {
            r.set_viewport(160, 160, 1.).unwrap();
        } else {
            r.apply(&[Mutation::Resume {
                id,
                parent: None,
                before: None,
            }])
            .unwrap();
        }
        r.drain(&mut inputs);
        assert!(!inputs.iter().any(|i| matches!(
            i,
            Input::Event {
                event: Event::PresenceComplete { .. },
                ..
            }
        )));
        assert_eq!(r.engine_id(id, glory_html::Part::Main).unwrap(), physical);
        r.apply(&[Mutation::Dispose { id }]).unwrap();
        r.advance_time(2.).unwrap();
        inputs.clear();
        r.drain(&mut inputs);
        assert!(inputs.is_empty());
        assert_eq!(r.stats().nodes, 0);
    }
}
