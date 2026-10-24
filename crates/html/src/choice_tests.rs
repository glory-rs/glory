use crate::*;
use glory_render::*;

#[derive(Default)]
struct Record(Vec<Op>);
impl Sink for Record {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        self.0.push(op);
        Ok(())
    }
}
fn name(value: &str) -> SemanticId {
    SemanticId::new(value).unwrap()
}
fn box_descriptor() -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::Listbox);
    d.semantics = Some(Box::new(Semantics {
        id: Some(name("box")),
        navigation: Some(Orientation::Vertical),
        ..Default::default()
    }));
    d
}
fn option_descriptor() -> Descriptor {
    let mut d = Descriptor::new(Props::Button(ButtonProps {
        label: "Option".into(),
        ..Default::default()
    }));
    d.focus = FocusMode::Programmatic;
    d.semantics = Some(Box::new(Semantics {
        id: Some(name("option")),
        option: Some(OptionSemantics { owner: name("box") }),
        selected: Some(false),
        ..Default::default()
    }));
    d
}

#[test]
fn select_typeahead_without_navigation_has_a_real_host_interest_attribute_and_delta() {
    let mut base = Descriptor::new(Props::Button(ButtonProps::default()));
    base.semantics = Some(Box::new(Semantics {
        id: Some(name("trigger")),
        combobox: Some(ComboboxKind::Select),
        controls: Some(name("box")),
        popup: Some(PopupKind::Listbox),
        expanded: Some(false),
        typeahead: true,
        ..Default::default()
    }));
    let before = shape(&base).unwrap();
    assert!(before.pieces.iter().any(|piece| matches!(&piece.kind, Kind::Element {attributes, ..} if attributes.contains_key("data-gly-typeahead") && !attributes.contains_key("data-gly-navigation"))));
    let mut metadata = base.semantics.clone().unwrap();
    metadata.typeahead = false;
    let patch = Patch::Semantics(Change::Set(Some(metadata)));
    let after = shape(&base.patched(patch.clone()).unwrap()).unwrap();
    let expected = crate::difference(&before, &after, false, false, false).unwrap();
    assert!(expected.iter().any(|edit| matches!(
        edit,
        Edit::Attribute {
            name: "data-gly-typeahead",
            value: None,
            ..
        }
    )));
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    mapper
        .apply(
            &[
                Mutation::Create {
                    id,
                    node: Box::new(base),
                },
                Mutation::Place {
                    id,
                    parent: None,
                    before: None,
                },
            ],
            &mut sink,
        )
        .unwrap();
    sink.0.clear();
    mapper
        .apply(&[Mutation::Patch { id, patch }], &mut sink)
        .unwrap();
    assert!(matches!(&sink.0[0], Op::Patch {edits, ..} if edits == &expected));
}

#[test]
fn every_choice_semantics_delta_matches_the_full_shape_oracle_with_option_ownership() {
    for base in [
        option_descriptor(),
        box_descriptor(),
        {
            let mut d = Descriptor::new(Props::Button(ButtonProps::default()));
            d.semantics = Some(Box::new(Semantics {
                id: Some(name("trigger")),
                combobox: Some(ComboboxKind::Select),
                controls: Some(name("box")),
                popup: Some(PopupKind::Listbox),
                expanded: Some(false),
                typeahead: true,
                ..Default::default()
            }));
            d
        },
        {
            let mut d = Descriptor::new(Props::Input(InputProps {
                value: "large preserved input".repeat(100),
                revision: 23,
                ..Default::default()
            }));
            d.semantics = Some(Box::new(Semantics {
                id: Some(name("editor")),
                combobox: Some(ComboboxKind::Editable),
                controls: Some(name("box")),
                popup: Some(PopupKind::Listbox),
                expanded: Some(false),
                ..Default::default()
            }));
            d
        },
    ] {
        let original = base.semantics.clone().unwrap();
        let mut next = original.clone();
        if next.option.is_some() {
            next.selected = Some(true);
            next.disabled = true;
            next.hidden = true;
        }
        if next.combobox.is_some() {
            next.expanded = Some(true);
            next.active_descendant = Some(name("future-option"));
        }
        if base.role == Some(Role::Listbox) {
            next.listbox_multiple = true;
            next.layer = Some(Layer {
                modal: false,
                focus: LayerFocus::Preserve,
                placement: LayerPlacement::AnchorStart,
                anchor: Some(name("future-trigger")),
                ..Default::default()
            });
        }
        for patch in [
            Patch::Semantics(Change::Keep),
            Patch::Semantics(Change::Set(Some(next))),
            Patch::Semantics(Change::Set(Some(original))),
            Patch::Name(Change::Set(Some("New name".into()))),
            Patch::Enabled(Change::Set(false)),
        ] {
            let Ok(next) = base.patched(patch.clone()) else {
                continue;
            };
            let expected = crate::difference(
                &shape(&base).unwrap(),
                &shape(&next).unwrap(),
                false,
                false,
                false,
            )
            .unwrap();
            let runtime = RuntimeId::new();
            let mut ids = NodeIds::new(runtime);
            let id = ids.allocate();
            let owner = ids.allocate();
            let mut mapper = Mapper::new(runtime);
            let mut sink = Record::default();
            let option = base.semantics.as_ref().is_some_and(|s| s.option.is_some());
            if option {
                mapper
                    .apply(
                        &[
                            Mutation::Create {
                                id: owner,
                                node: Box::new(box_descriptor()),
                            },
                            Mutation::Place {
                                id: owner,
                                parent: None,
                                before: None,
                            },
                        ],
                        &mut sink,
                    )
                    .unwrap();
            }
            mapper
                .apply(
                    &[
                        Mutation::Create {
                            id,
                            node: Box::new(base.clone()),
                        },
                        Mutation::Place {
                            id,
                            parent: option.then_some(owner),
                            before: None,
                        },
                    ],
                    &mut sink,
                )
                .unwrap();
            sink.0.clear();
            mapper
                .apply(&[Mutation::Patch { id, patch }], &mut sink)
                .unwrap();
            assert!(matches!(&sink.0[0],Op::Patch{edits,..}if edits==&expected));
            if option {
                let before = mapper.tree().stats();
                assert!(
                    mapper
                        .apply(
                            &[Mutation::Patch {
                                id,
                                patch: Patch::Semantics(Change::Clear)
                            }],
                            &mut sink
                        )
                        .is_err()
                );
                assert_eq!(mapper.tree().stats(), before);
            }
        }
    }
}

#[test]
fn editable_choice_modes_required_and_defaults_match_full_shape_delta_oracle() {
    for kind in [
        ComboboxKind::Editable,
        ComboboxKind::Autocomplete,
        ComboboxKind::Multiple,
    ] {
        let mut base = Descriptor::new(Props::Input(InputProps {
            value: "preserved 😀 editor".repeat(80),
            revision: 7,
            ..Default::default()
        }));
        base.semantics = Some(Box::new(Semantics {
            id: Some(name("editor")),
            combobox: Some(kind),
            controls: Some(name("later-box")),
            popup: Some(PopupKind::Listbox),
            expanded: Some(false),
            required: true,
            readonly: true,
            ..Default::default()
        }));
        let before = shape(&base).unwrap();
        let main = before
            .pieces
            .iter()
            .find_map(|piece| match &piece.kind {
                Kind::Element { attributes, .. } => Some(attributes),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            main.contains_key("required"),
            kind != ComboboxKind::Multiple
        );
        assert!(main.contains_key("readonly"));
        let mut metadata = base.semantics.clone().unwrap();
        metadata.combobox = Some(if kind == ComboboxKind::Multiple {
            ComboboxKind::Autocomplete
        } else {
            ComboboxKind::Multiple
        });
        for patch in [
            Patch::Semantics(Change::Keep),
            Patch::Semantics(Change::Set(Some(metadata))),
            Patch::Semantics(Change::Clear),
            Patch::Semantics(Change::Set(None)),
        ] {
            let next = base.patched(patch.clone()).unwrap();
            let expected =
                crate::difference(&before, &shape(&next).unwrap(), false, false, false).unwrap();
            let runtime = RuntimeId::new();
            let id = NodeIds::new(runtime).allocate();
            let mut mapper = Mapper::new(runtime);
            let mut sink = Record::default();
            mapper
                .apply(
                    &[
                        Mutation::Create {
                            id,
                            node: Box::new(base.clone()),
                        },
                        Mutation::Place {
                            id,
                            parent: None,
                            before: None,
                        },
                    ],
                    &mut sink,
                )
                .unwrap();
            sink.0.clear();
            mapper
                .apply(&[Mutation::Patch { id, patch }], &mut sink)
                .unwrap();
            assert!(matches!(&sink.0[0],Op::Patch{edits,..}if edits==&expected));
            let Props::Input(input) = mapper.tree().node(id).unwrap().descriptor.view().props
            else {
                panic!()
            };
            assert_eq!(input.revision, 7);
            assert_eq!(input.value, "preserved 😀 editor".repeat(80));
        }
    }
}
