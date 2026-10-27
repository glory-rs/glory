#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;

#[test]
fn combobox_kinds_and_options_admit_through_preflight_and_direct_apply_with_shared_owner_validation()
 {
    for kind in [
        ComboboxKind::Select,
        ComboboxKind::Editable,
        ComboboxKind::Autocomplete,
        ComboboxKind::Multiple,
    ] {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let trigger = ids.allocate();
        let panel = ids.allocate();
        let option = ids.allocate();
        let mut renderer = Renderer::new(runtime);
        let mut d = Descriptor::new(if kind == ComboboxKind::Select {
            Props::Button(Default::default())
        } else {
            Props::Input(Default::default())
        });
        d.semantics = Some(Box::new(Semantics {
            id: Some(SemanticId::new("trigger").unwrap()),
            combobox: Some(kind),
            controls: Some(SemanticId::new("panel").unwrap()),
            popup: Some(PopupKind::Listbox),
            expanded: Some(false),
            ..Default::default()
        }));
        renderer.preflight(d.view(), None).unwrap();
        renderer
            .preflight(
                d.view(),
                Some(&Patch::Semantics(Change::Set(d.semantics.clone()))),
            )
            .unwrap();
        let mut box_node = Descriptor::new(Props::View);
        box_node.role = Some(Role::Listbox);
        box_node.semantics = Some(Box::new(Semantics {
            id: Some(SemanticId::new("panel").unwrap()),
            navigation: Some(Orientation::Vertical),
            listbox_multiple: kind == ComboboxKind::Multiple,
            ..Default::default()
        }));
        let mut item = Descriptor::new(Props::Button(Default::default()));
        item.focus = FocusMode::Programmatic;
        item.semantics = Some(Box::new(Semantics {
            id: Some(SemanticId::new("option").unwrap()),
            option: Some(OptionSemantics {
                owner: SemanticId::new("panel").unwrap(),
            }),
            selected: Some(false),
            ..Default::default()
        }));
        renderer.preflight(item.view(), None).unwrap();
        renderer
            .apply(&[
                Mutation::Create {
                    id: trigger,
                    node: Box::new(d),
                },
                Mutation::Place {
                    id: trigger,
                    parent: None,
                    before: None,
                },
                Mutation::Create {
                    id: panel,
                    node: Box::new(box_node),
                },
                Mutation::Place {
                    id: panel,
                    parent: None,
                    before: None,
                },
                Mutation::Create {
                    id: option,
                    node: Box::new(item),
                },
                Mutation::Place {
                    id: option,
                    parent: Some(panel),
                    before: None,
                },
            ])
            .unwrap();
        renderer.listen(trigger, EventKind::Choice, true).unwrap();
        renderer.listen(option, EventKind::Pointer, true).unwrap();
        let physical = renderer.engine_id(trigger, glory_html::Part::Main).unwrap();
        let writes = renderer.stats().property_writes;
        let mut inputs = Vec::new();
        renderer
            .request(Query::Bounds(option), QueryToken(1))
            .unwrap();
        renderer.drain(&mut inputs);
        assert!(matches!(
            inputs.as_slice(),
            [Input::Response {
                applied: 6,
                result: Ok(QueryValue::Bounds(_)),
                ..
            }]
        ));
        renderer
            .action(Action::RevealOption { node: option })
            .unwrap();
        assert_eq!(
            renderer.engine_id(trigger, glory_html::Part::Main).unwrap(),
            physical
        );
        assert_eq!(renderer.stats().property_writes, writes);
        assert!(!renderer.capabilities().input);
        assert!(renderer.action(Action::Focus(option)).is_err());
        // Ownership remains a shared tree invariant after native admission.
        assert!(
            renderer
                .apply(&[Mutation::Place {
                    id: option,
                    parent: None,
                    before: None
                }])
                .is_err()
        );
        assert_eq!(renderer.stats().applied, 6);
    }
}
