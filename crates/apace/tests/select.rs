#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_html::Part;
use glory_render::*;

#[test]
fn native_select_admission_selection_barriers_and_later_rejection_keep_prefix_and_identity() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let rejected = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let key = SemanticId::new("a").unwrap();
    let descriptor = Descriptor::new(Props::Select(SelectProps {
        entries: std::rc::Rc::new(vec![SelectEntry::Option(SelectOption {
            key: key.clone(),
            label: "中文".into(),
            value: "business-value".into(),
            disabled: false,
        })]),
        ..Default::default()
    }));
    renderer.preflight(descriptor.view(), None).unwrap();
    renderer
        .preflight(
            descriptor.view(),
            Some(&Patch::SelectSelection {
                keys: Change::Set(vec![key.clone()]),
                revision: 1,
            }),
        )
        .unwrap();
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id,
                node: Box::new(descriptor)
            },
            Mutation::Place {
                id,
                parent: None,
                before: None
            },
            Mutation::Patch {
                id,
                patch: Patch::SelectSelection {
                    keys: Change::Set(vec![key.clone()]),
                    revision: 1
                }
            },
            Mutation::Create {
                id: rejected,
                node: Box::new(Descriptor::new(Props::File(Default::default())))
            },
        ]),
        Err(Error::Unsupported("native file picker controls"))
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 3);
    let physical = renderer.engine_id(id, Part::Main).unwrap();
    renderer
        .request(Query::Bounds(id), QueryToken(701))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(
        matches!(inputs.as_slice(),[Input::Response {token:QueryToken(701),applied:3,result:Ok(QueryValue::Bounds(rect))}] if rect.width>0. && rect.height>0.)
    );
    renderer.apply(&[Mutation::Suspend { id }]).unwrap();
    renderer
        .apply(&[Mutation::Resume {
            id,
            parent: None,
            before: None,
        }])
        .unwrap();
    let before = renderer.stats().property_writes;
    renderer
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::SelectSelection {
                keys: Change::Keep,
                revision: 99,
            },
        }])
        .unwrap();
    assert_eq!(renderer.stats().property_writes, before);
    renderer
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::SelectSelection {
                keys: Change::Set(vec![key]),
                revision: 2,
            },
        }])
        .unwrap();
    assert_eq!(renderer.engine_id(id, Part::Main).unwrap(), physical);
    assert_eq!(renderer.stats().property_writes, before + 1);
}

#[test]
fn native_select_explicit_plan_instances_keep_independent_live_selection_and_resume_identity() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let one = ids.allocate();
    let two = ids.allocate();
    let key = SemanticId::new("option").unwrap();
    let entries = std::rc::Rc::new(vec![SelectEntry::Option(SelectOption {
        key: key.clone(),
        label: "中文".into(),
        value: "secret".into(),
        disabled: false,
    })]);
    let plan = Plan::new(vec![PlanNode {
        parent: None,
        descriptor: Descriptor::new(Props::Select(SelectProps {
            entries,
            ..Default::default()
        })),
    }])
    .unwrap();
    let mut renderer = Renderer::new(runtime);
    for id in [one, two] {
        renderer
            .apply(&[
                Mutation::CreatePlan {
                    plan: plan.clone(),
                    ids: vec![id],
                },
                Mutation::Place {
                    id,
                    parent: None,
                    before: None,
                },
            ])
            .unwrap();
    }
    let first = renderer.engine_id(one, Part::Main).unwrap();
    renderer
        .apply(&[
            Mutation::Patch {
                id: one,
                patch: Patch::SelectSelection {
                    keys: Change::Set(vec![key]),
                    revision: 1,
                },
            },
            Mutation::Suspend { id: one },
            Mutation::Resume {
                id: one,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    assert_eq!(renderer.engine_id(one, Part::Main).unwrap(), first);
    let second = renderer.engine_id(two, Part::Main).unwrap();
    renderer.inspect(|doc| {
        let label = |main| {
            let caption = doc.get_node(main).unwrap().children[0];
            let text = doc.get_node(caption).unwrap().children[0];
            doc.get_node(text)
                .unwrap()
                .text_data()
                .unwrap()
                .content
                .clone()
        };
        assert_eq!(label(first), "中文");
        assert_eq!(label(second), "");
    });
}
