use crate::*;

/// Descriptor-local rejection and accepted-prefix/query consistency for the
/// optional collapse facet. Completion capability is a separate host promise:
/// SSR may store the finite configuration without observing physical motion.
pub fn verify_collapse(mut renderer: impl Render, runtime: RuntimeId, supported: bool) {
    let mut ids = NodeIds::new(runtime);
    let plain = ids.allocate();
    let motion = ids.allocate();
    let bad = ids.allocate();
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.semantics = Some(Box::new(Semantics {
        collapse: Some(CollapseConfig {
            open: false,
            duration_millis: 240,
            revision: 2,
        }),
        hidden: true,
        inert: true,
        ..Default::default()
    }));
    renderer
        .apply(&[
            Mutation::Create {
                id: plain,
                node: Box::new(Descriptor::new(Props::View)),
            },
            Mutation::Place {
                id: plain,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let preflight = renderer.preflight(descriptor.view(), None);
    if !supported {
        assert!(preflight.is_err());
        assert!(
            renderer
                .apply(&[Mutation::Create {
                    id: motion,
                    node: Box::new(descriptor)
                }])
                .is_err()
        );
        renderer
            .request(Query::Bounds(plain), QueryToken(1))
            .unwrap();
        let mut inputs = Vec::new();
        renderer.drain(&mut inputs);
        assert!(inputs.iter().any(|input| matches!(
            input,
            Input::Response {
                token: QueryToken(1),
                applied: 2,
                ..
            }
        )));
        return;
    }
    preflight.unwrap();
    renderer
        .apply(&[
            Mutation::Create {
                id: motion,
                node: Box::new(descriptor.clone()),
            },
            Mutation::Place {
                id: motion,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    renderer
        .listen(motion, EventKind::CollapseComplete, true)
        .unwrap();
    renderer
        .request(Query::Bounds(plain), QueryToken(2))
        .unwrap();
    let mut next = descriptor.clone();
    let metadata = next.semantics.as_mut().unwrap();
    metadata.hidden = false;
    metadata.inert = false;
    metadata.collapse = Some(CollapseConfig {
        open: true,
        duration_millis: 0,
        revision: 3,
    });
    let mut invalid = descriptor;
    invalid
        .semantics
        .as_mut()
        .unwrap()
        .collapse
        .as_mut()
        .unwrap()
        .duration_millis = 501;
    // Shared descriptor rules precede adapter-local preflight. The latter
    // receives an already validated staged descriptor from the App.
    assert!(invalid.validate().is_err());
    assert!(
        renderer
            .apply(&[
                Mutation::Patch {
                    id: motion,
                    patch: Patch::Semantics(Change::Set(next.semantics))
                },
                Mutation::Create {
                    id: bad,
                    node: Box::new(invalid)
                }
            ])
            .is_err()
    );
    renderer
        .request(Query::Bounds(plain), QueryToken(3))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(inputs.iter().any(|input| matches!(
        input,
        Input::Response {
            token: QueryToken(2),
            applied: 4,
            ..
        }
    )));
    assert!(inputs.iter().any(|input| matches!(
        input,
        Input::Response {
            token: QueryToken(3),
            applied: 5,
            ..
        }
    )));
}
