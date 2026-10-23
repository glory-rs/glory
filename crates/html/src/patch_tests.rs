use super::*;
use glory_render::*;

#[derive(Default)]
struct Record(Vec<Op>);
impl Sink for Record {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        self.0.push(op);
        Ok(())
    }
}

#[test]
fn every_field_delta_matches_full_shape_difference_including_resets_and_normalization() {
    let mut bases = vec![
        Props::View,
        Props::Group,
        Props::Text(TextProps {
            content: "old".into(),
            ..Default::default()
        }),
        Props::Text(TextProps {
            content: "old".into(),
            kind: TextKind::Heading(2),
            selectable: false,
        }),
        Props::Button(ButtonProps {
            label: "old".into(),
            ..Default::default()
        }),
        Props::Input(InputProps {
            value: "old".into(),
            revision: 10,
            ..Default::default()
        }),
        Props::Input(InputProps {
            value: "old".into(),
            revision: 10,
            multiline: true,
            ..Default::default()
        }),
        Props::Check(CheckProps {
            label: "old".into(),
            ..Default::default()
        }),
        Props::Check(CheckProps {
            mode: CheckMode::Mixed,
            checked: true,
            ..Default::default()
        }),
        Props::Check(CheckProps {
            mode: CheckMode::Switch,
            checked: true,
            ..Default::default()
        }),
        Props::Check(CheckProps {
            mode: CheckMode::Radio,
            checked: true,
            ..Default::default()
        }),
        Props::Image(Default::default()),
        Props::Image(ImageProps {
            source: "old.png".into(),
            alternative: Some("old".into()),
        }),
        Props::Link(LinkProps {
            target: "#old".into(),
            label: "old".into(),
        }),
    ]
    .into_iter()
    .map(Descriptor::new)
    .collect::<Vec<_>>();
    let mut overridden = Descriptor::new(Props::Text(TextProps {
        content: "old".into(),
        ..Default::default()
    }));
    overridden
        .options
        .insert(HtmlTextOptions::default().tag(TextTag::P));
    bases.push(overridden);
    for kind in [TextKind::Code, TextKind::Kbd, TextKind::Mark] {
        bases.push(Descriptor::new(Props::Text(TextProps {
            content: "old".into(),
            kind,
            ..Default::default()
        })));
    }
    for part in [
        DocumentPart::Blockquote,
        DocumentPart::OrderedList,
        DocumentPart::UnorderedList,
        DocumentPart::DescriptionList,
    ] {
        let mut descriptor = Descriptor::new(Props::View);
        descriptor.semantics = Some(Box::new(Semantics {
            document_part: Some(part),
            ..Default::default()
        }));
        bases.push(descriptor);
    }
    let mut named = Descriptor::new(Props::View);
    named.name = Some("old\0".into());
    bases.push(named);
    let mut item = Descriptor::new(Props::View);
    item.role = Some(Role::ListItem);
    item.item = Some(ItemPosition {
        index: 3,
        count: Some(100_000_000),
    });
    bases.push(item);
    let mut fieldset = Descriptor::new(Props::View);
    fieldset.semantics = Some(Box::new(Semantics {
        field_part: Some(FieldPart::Fieldset),
        ..Default::default()
    }));
    bases.push(fieldset);
    let mut patches = vec![
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            field_part: Some(FieldPart::Fieldset),
            invalid: true,
            error_message: Some(SemanticId::new("group-error").unwrap()),
            ..Default::default()
        })))),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            check_group: Some(SemanticId::new("radio-association").unwrap()),
            navigation_item: true,
            ..Default::default()
        })))),
        Patch::Semantics(Change::Keep),
        Patch::Semantics(Change::Clear),
        Patch::Semantics(Change::Set(None)),
        Patch::Text(Change::Keep),
        Patch::Text(Change::Set("old".into())),
        Patch::Text(Change::Set("new\r\n\0🙂".into())),
        Patch::Text(Change::Clear),
        Patch::TextKind(Change::Keep),
        Patch::TextKind(Change::Set(TextKind::Paragraph)),
        Patch::TextKind(Change::Set(TextKind::Heading(1))),
        Patch::TextKind(Change::Clear),
        Patch::Selectable(Change::Keep),
        Patch::Selectable(Change::Set(false)),
        Patch::Selectable(Change::Set(true)),
        Patch::Selectable(Change::Clear),
        Patch::Label(Change::Keep),
        Patch::Label(Change::Set("new\r\n\0🙂".into())),
        Patch::Label(Change::Clear),
        Patch::Enabled(Change::Keep),
        Patch::Enabled(Change::Set(false)),
        Patch::Enabled(Change::Set(true)),
        Patch::Enabled(Change::Clear),
        Patch::Value {
            value: Change::Keep,
            revision: u64::MAX,
        },
        Patch::Value {
            value: Change::Set("old".into()),
            revision: 10,
        },
        Patch::Value {
            value: Change::Set("old".into()),
            revision: 11,
        },
        Patch::Value {
            value: Change::Set("new".into()),
            revision: 10,
        },
        Patch::Value {
            value: Change::Set("new\r\n\0🙂".into()),
            revision: 11,
        },
        Patch::Value {
            value: Change::Clear,
            revision: 11,
        },
        Patch::Multiline(Change::Keep),
        Patch::Multiline(Change::Set(false)),
        Patch::Multiline(Change::Set(true)),
        Patch::Multiline(Change::Clear),
        Patch::CheckMode(Change::Keep),
        Patch::CheckMode(Change::Set(CheckMode::Checkbox)),
        Patch::CheckMode(Change::Set(CheckMode::Mixed)),
        Patch::CheckMode(Change::Set(CheckMode::Switch)),
        Patch::CheckMode(Change::Set(CheckMode::Radio)),
        Patch::CheckMode(Change::Clear),
        Patch::Checked(Change::Keep),
        Patch::Checked(Change::Set(false)),
        Patch::Checked(Change::Set(true)),
        Patch::Checked(Change::Clear),
        Patch::Source(Change::Keep),
        Patch::Source(Change::Set("new\0.png".into())),
        Patch::Source(Change::Clear),
        Patch::Alternative(Change::Keep),
        Patch::Alternative(Change::Set(Some("new\r\n\0".into()))),
        Patch::Alternative(Change::Set(Some(String::new()))),
        Patch::Alternative(Change::Set(None)),
        Patch::Alternative(Change::Clear),
        Patch::Target(Change::Keep),
        Patch::Target(Change::Set("#new\0".into())),
        Patch::Target(Change::Clear),
        Patch::Classes(Change::Keep),
        Patch::Classes(Change::Set(vec!["new\0".into()])),
        Patch::Classes(Change::Set(vec!["old".into()])),
        Patch::Classes(Change::Set(vec!["bad token".into()])),
        Patch::Classes(Change::Clear),
        Patch::Style(Change::Keep),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().text_overflow(style::TextOverflow::Clip),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().text_overflow(style::TextOverflow::Ellipsis),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().flex_wrap(style::FlexWrap::Wrap),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().flex_wrap(style::FlexWrap::NoWrap),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().box_shadow(style::BoxShadow::None),
        ))),
        Patch::Style(Change::Set(Box::new(style::Style::new().box_shadow(
            style::BoxShadow::Outset(style::Shadow {
                x: -2.0,
                y: 3.0,
                blur: 4.0,
                spread: -1.0,
                color: style::Color(10, 20, 30, 40),
            }),
        )))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().field_sizing(style::FieldSizing::Content),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().field_sizing(style::FieldSizing::Fixed),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().width(style::Length::Px(-0.0)),
        ))),
        Patch::Style(Change::Set(Box::new(style::Style {
            color: Some(style::Color(1, 2, 3, 255)),
            ..Default::default()
        }))),
        Patch::Style(Change::Clear),
        Patch::Name(Change::Keep),
        Patch::Name(Change::Set(Some("new\r\n\0".into()))),
        Patch::Name(Change::Set(Some(String::new()))),
        Patch::Name(Change::Set(Some("old�".into()))),
        Patch::Name(Change::Set(None)),
        Patch::Name(Change::Clear),
        Patch::Focus(Change::Keep),
        Patch::Focus(Change::Set(FocusMode::Programmatic)),
        Patch::Focus(Change::Set(FocusMode::TabStop)),
        Patch::Focus(Change::Clear),
        Patch::Item(Change::Keep),
        Patch::Item(Change::Set(Some(ItemPosition {
            index: 99_999_999,
            count: None,
        }))),
        Patch::Item(Change::Set(Some(ItemPosition {
            index: usize::MAX,
            count: None,
        }))),
        Patch::Item(Change::Set(None)),
        Patch::Item(Change::Clear),
        Patch::Role(Change::Keep),
        Patch::Role(Change::Set(None)),
        Patch::Role(Change::Clear),
        Patch::Purpose(Change::Keep),
        Patch::Purpose(Change::Clear),
    ];
    for props in [
        Props::Button(ButtonProps::default()),
        Props::Check(CheckProps::default()),
    ] {
        let mut named = Descriptor::new(props);
        named.name = Some("named control".into());
        bases.push(named);
    }
    let semantic_cases = [
        Semantics {
            link_open: Some(glory_render::LinkOpen::Current),
            ..Default::default()
        },
        Semantics {
            link_open: Some(glory_render::LinkOpen::NewContext),
            ..Default::default()
        },
        Semantics {
            pressed: Some(false),
            ..Default::default()
        },
        Semantics {
            pressed: Some(true),
            ..Default::default()
        },
        Semantics {
            field_part: Some(FieldPart::Form),
            ..Default::default()
        },
        Semantics {
            field_part: Some(FieldPart::Form),
            labelled_by: Some(SemanticId::new("form-label").unwrap()),
            ..Default::default()
        },
        Semantics {
            form_action: Some(FormAction::Submit),
            form_control: Some(FormControl {
                name: "intent".into(),
                value: Some("save".into()),
            }),
            ..Default::default()
        },
        Semantics {
            form_action: Some(FormAction::Reset),
            ..Default::default()
        },
        Semantics {
            form_control: Some(FormControl {
                name: "value".into(),
                value: None,
            }),
            ..Default::default()
        },
        Semantics {
            required: true,
            readonly: true,
            invalid: true,
            described_by: Some(SemanticId::new("description").unwrap()),
            error_message: Some(SemanticId::new("error").unwrap()),
            ..Default::default()
        },
        Semantics {
            field_part: Some(FieldPart::Label),
            label_for: Some(SemanticId::new("target").unwrap()),
            ..Default::default()
        },
        Semantics {
            field_part: Some(FieldPart::Fieldset),
            disabled: true,
            ..Default::default()
        },
        Semantics {
            id: Some(SemanticId::new("layer").unwrap()),
            described_by: Some(SemanticId::new("description").unwrap()),
            layer: Some(Layer::default()),
            ..Default::default()
        },
        Semantics {
            has_popup: true,
            ..Default::default()
        },
        Semantics::default(),
        Semantics {
            id: Some(SemanticId::new("node").unwrap()),
            labelled_by: Some(SemanticId::new("label").unwrap()),
            hidden: true,
            ..Default::default()
        },
        Semantics {
            expanded: Some(false),
            controls: Some(SemanticId::new("panel").unwrap()),
            navigation_item: true,
            ..Default::default()
        },
        Semantics {
            expanded: Some(true),
            disabled: true,
            ..Default::default()
        },
        Semantics {
            tab: true,
            selected: Some(false),
            controls: Some(SemanticId::new("panel").unwrap()),
            ..Default::default()
        },
        Semantics {
            tab: true,
            selected: Some(true),
            ..Default::default()
        },
        Semantics {
            navigation: Some(Orientation::Horizontal),
            ..Default::default()
        },
        Semantics {
            navigation: Some(Orientation::Vertical),
            ..Default::default()
        },
    ];
    for metadata in semantic_cases {
        patches.push(Patch::Semantics(Change::Set(Some(Box::new(
            metadata.clone(),
        )))));
        for props in [
            Props::Link(LinkProps::default()),
            Props::View,
            Props::Button(ButtonProps::default()),
            Props::Input(InputProps::default()),
            Props::Check(CheckProps::default()),
        ] {
            let mut base = Descriptor::new(props);
            if metadata.layer.is_some() && matches!(base.props, Props::View) {
                base.role = Some(Role::Dialog);
                base.focus = FocusMode::Programmatic;
            }
            base.semantics = Some(Box::new(metadata.clone()));
            if base.validate().is_ok() {
                bases.push(base);
            }
        }
    }
    for role in [
        Role::TabList,
        Role::TabPanel,
        Role::RadioGroup,
        Role::Heading(1),
        Role::Heading(6),
        Role::Heading(0),
        Role::Heading(7),
    ] {
        patches.push(Patch::Role(Change::Set(Some(role))));
        let mut base = Descriptor::new(Props::View);
        base.role = Some(role);
        if base.validate().is_ok() {
            bases.push(base);
        }
    }
    patches.extend(
        [
            Purpose::Text,
            Purpose::Password,
            Purpose::Email,
            Purpose::Url,
            Purpose::Number,
            Purpose::Search,
            Purpose::Tel,
        ]
        .into_iter()
        .map(|purpose| Patch::Purpose(Change::Set(purpose))),
    );
    patches.extend(
        [
            Role::Group,
            Role::Main,
            Role::Navigation,
            Role::Section,
            Role::List,
            Role::ListItem,
            Role::Status,
            Role::Alert,
            Role::Table,
            Role::Row,
            Role::ColumnHeader,
            Role::Cell,
            Role::Dialog,
        ]
        .into_iter()
        .map(|role| Patch::Role(Change::Set(Some(role)))),
    );
    let (mut accepted, mut rejected) = (0, 0);
    for limit in [
        style::Limit::None,
        style::Limit::Px(25.0),
        style::Limit::Percent(50.0),
        style::Limit::Px(-1.0),
        style::Limit::Px(f64::NAN),
        style::Limit::Percent(f64::INFINITY),
    ] {
        patches.push(Patch::Style(Change::Set(Box::new(
            style::Style::new().max_width(limit).max_height(limit),
        ))));
    }
    for mut base in bases {
        if !matches!(base.props, Props::Group) {
            base.classes = vec!["old".into()];
            base.style = style::Style {
                width: Some(style::Length::Px(0.0)),
                background: Some(style::Color(8, 9, 10, 255)),
                ..Default::default()
            };
        }
        for update in &patches {
            let expected = base.patched(update.clone()).and_then(|next| {
                difference(
                    &shape(&base)?,
                    &shape(&next)?,
                    matches!(update, Patch::Checked(Change::Set(_) | Change::Clear)),
                    matches!(update, Patch::CheckMode(Change::Set(_) | Change::Clear)),
                )
            });
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
            let receipt = mapper.receipt();
            let stats = mapper.tree().stats();
            let ops = sink.0.len();
            let actual = mapper.apply(
                &[Mutation::Patch {
                    id,
                    patch: update.clone(),
                }],
                &mut sink,
            );
            match expected {
                Ok(expected) => {
                    accepted += 1;
                    assert_eq!(actual.unwrap().applied, receipt.applied + 1);
                    let Some(Op::Patch { edits, .. }) = sink.0.last() else {
                        panic!()
                    };
                    assert_eq!(edits, &expected, "{:?}/{update:?}", base.props);
                }
                Err(_) => {
                    rejected += 1;
                    assert!(actual.is_err(), "{:?}/{update:?}", base.props);
                    assert_eq!(mapper.tree().node(id).unwrap().descriptor, base);
                    assert_eq!(mapper.tree().stats(), stats);
                    assert_eq!(mapper.receipt(), receipt);
                    assert_eq!(sink.0.len(), ops);
                }
            }
        }
    }
    assert!(accepted > 300 && rejected > 300);
    println!("field delta oracle: accepted={accepted}, rejected={rejected}");
}

#[test]
fn mapper_metadata_keeps_large_unrelated_payloads_and_emits_no_value_property() {
    for props in [
        Props::Input(InputProps {
            value: "x".repeat(1_048_576),
            ..Default::default()
        }),
        Props::Text(TextProps {
            content: "x".repeat(1_048_576),
            kind: TextKind::Heading(2),
            ..Default::default()
        }),
    ] {
        let runtime = RuntimeId::new();
        let id = NodeIds::new(runtime).allocate();
        let mut mapper = Mapper::new(runtime);
        let mut sink = Record::default();
        mapper
            .apply(
                &[
                    Mutation::Create {
                        id,
                        node: Box::new(Descriptor::new(props)),
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
        let pointer = |mapper: &Mapper| match &mapper.tree().node(id).unwrap().descriptor.props {
            Props::Input(input) => input.value.as_ptr(),
            Props::Text(text) => text.content.as_ptr(),
            _ => unreachable!(),
        };
        let original = pointer(&mapper);
        for patch in [
            Patch::Name(Change::Set(Some("Name".into()))),
            Patch::Name(Change::Keep),
            Patch::Name(Change::Clear),
            Patch::Focus(Change::Set(FocusMode::Programmatic)),
        ] {
            mapper
                .apply(&[Mutation::Patch { id, patch }], &mut sink)
                .unwrap();
            assert_eq!(pointer(&mapper), original);
            let Some(Op::Patch { edits, .. }) = sink.0.last() else {
                panic!()
            };
            assert!(
                edits
                    .iter()
                    .all(|edit| matches!(edit, Edit::Attribute { .. }))
            );
        }
    }
}
