//! The legacy key writer is intentionally independent of the shared projection:
//! full-shape differences alone could otherwise hide a shared precedence bug.
use crate::*;
use glory_render::*;
use std::collections::BTreeMap;

fn roles() -> Vec<Option<Role>> {
    use Role::*;
    let mut roles = vec![None];
    roles.extend(
        [
            Slider,
            SpinButton,
            ProgressBar,
            Separator,
            Group,
            Main,
            Banner,
            ContentInfo,
            Section,
            Navigation,
            List,
            ListItem,
            Status,
            Alert,
            Table,
            Grid,
            GridCell,
            Tree,
            TreeItem,
            Row,
            ColumnHeader,
            Cell,
            Dialog,
            AlertDialog,
            Tooltip,
            TabList,
            TabPanel,
            RadioGroup,
            Toolbar,
            Menu,
            Menubar,
            Listbox,
        ]
        .map(Some),
    );
    roles.extend((0..=7).map(|level| Some(Heading(level))));
    roles
}

fn legacy(value: Option<&Semantics>, role: Option<Role>) -> BTreeMap<&'static str, String> {
    let mut attrs = BTreeMap::new();
    if let Some(role) = role {
        attrs.insert("role", crate::mapping::role_name(role).into());
        if let Role::Heading(level) = role {
            attrs.insert("aria-level", level.to_string());
        }
    }
    let Some(value) = value else { return attrs };
    if let Some(tree) = value.tree_navigation {
        attrs.insert("aria-multiselectable", tree.multiple.to_string());
    }
    if let Some(item) = value.tree_item {
        attrs.insert("aria-level", item.level.to_string());
    }
    if value.combobox.is_some() {
        attrs.insert("role", "combobox".into());
    }
    if value.option.is_some() {
        attrs.insert("role", "option".into());
    }
    if role == Some(Role::Grid) {
        attrs.insert("aria-multiselectable", value.grid_multiple.to_string());
    }
    if role == Some(Role::Listbox) {
        attrs.insert("aria-multiselectable", value.listbox_multiple.to_string());
    }
    if let Some(axis) = value.separator_axis {
        attrs.insert("aria-orientation", axis.as_str().into());
    }
    if let Some(slider) = &value.slider {
        attrs.insert("aria-orientation", slider.axis.as_str().into());
    }
    if value.tab {
        attrs.insert("role", "tab".into());
    }
    if let Some(item) = &value.menu_item {
        attrs.insert(
            "role",
            match item.kind {
                MenuItemKind::Action => "menuitem",
                MenuItemKind::Checkbox => "menuitemcheckbox",
                MenuItemKind::Radio => "menuitemradio",
            }
            .into(),
        );
    }
    if let Some(layer) = &value.layer
        && matches!(role, Some(Role::Dialog | Role::AlertDialog))
    {
        attrs.insert("aria-modal", layer.modal.to_string());
    }
    if matches!(
        role,
        Some(Role::TabList | Role::RadioGroup | Role::Toolbar | Role::Menubar)
    ) && let Some(axis) = value.navigation
    {
        attrs.insert("aria-orientation", axis.as_str().into());
    }
    attrs
}

#[test]
fn projection_matches_independent_legacy_precedence_for_all_role_keys() {
    // Include overlapping facets even when construction would reject them. This
    // checks the pure formatter's complete override ordering independently of
    // validation, which the live Mapper matrix below exercises separately.
    for mask in 0usize..1024 {
        let enabled = |bit: usize| mask & (1usize << bit) != 0usize;
        let value = Semantics {
            tree_navigation: enabled(0).then_some(TreeNavigation {
                multiple: true,
                ..Default::default()
            }),
            tree_item: enabled(1).then_some(glory_render::TreeItem {
                level: 64,
                position: 0,
                count: 1,
            }),
            combobox: enabled(2).then_some(ComboboxKind::Select),
            option: enabled(3).then(|| OptionSemantics {
                owner: SemanticId::new("owner").unwrap(),
            }),
            tab: enabled(4),
            menu_item: enabled(5).then(|| MenuItemSemantics {
                owner: SemanticId::new("owner").unwrap(),
                kind: [
                    MenuItemKind::Action,
                    MenuItemKind::Checkbox,
                    MenuItemKind::Radio,
                ][mask % 3],
            }),
            separator_axis: enabled(6).then_some(Orientation::Vertical),
            slider: enabled(7).then_some(SliderValue {
                range: RangeValue {
                    min: 0.0,
                    max: 10.0,
                    value: 5.0,
                },
                axis: Orientation::Horizontal,
                direction: Direction::Ltr,
                step: 1.0,
                page_step: 2.0,
                value_text: Some("not role dependent".into()),
            }),
            layer: enabled(8).then(|| Layer {
                modal: enabled(0),
                ..Default::default()
            }),
            navigation: enabled(9).then_some(Orientation::Vertical),
            grid_multiple: enabled(1),
            listbox_multiple: enabled(2),
            ..Default::default()
        };
        for role in roles() {
            for value in [None, Some(&value)] {
                let projected = crate::semantics::role_attributes(value, role);
                assert!(projected.windows(2).all(|pair| pair[0].0 < pair[1].0));
                let projected: BTreeMap<_, _> = projected
                    .into_iter()
                    .filter_map(|(key, value)| {
                        value.map(|value| {
                            assert!(value.is_ascii());
                            (key, value.into_owned())
                        })
                    })
                    .collect();
                assert_eq!(projected, legacy(value, role), "mask={mask}/{role:?}");
                let full = crate::semantics::attributes(value, role);
                let full: BTreeMap<_, _> = full
                    .into_iter()
                    .filter(|(key, _)| {
                        [
                            "role",
                            "aria-level",
                            "aria-modal",
                            "aria-multiselectable",
                            "aria-orientation",
                        ]
                        .contains(key)
                    })
                    .collect();
                assert_eq!(projected, full);
            }
        }
    }
}

#[derive(Default)]
struct Record(Vec<Op>);
impl Sink for Record {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        self.0.push(op);
        Ok(())
    }
}

#[test]
fn role_transitions_match_full_shapes_and_rejections_preserve_state() {
    let mut bases = Vec::new();
    for role in roles() {
        for metadata in 0..7 {
            let mut base = Descriptor::new(Props::View);
            base.role = role;
            // Naming also exercises the implicit Group after Clear/Set(None).
            if metadata != 0 {
                base.name = Some("Name\r\nthat stays".repeat(100));
            }
            let mut value = Semantics::default();
            match metadata {
                1 => {
                    value.id = Some(SemanticId::new("identity").unwrap());
                    value.labelled_by = Some(SemanticId::new("label").unwrap());
                    value.described_by = Some(SemanticId::new("description").unwrap());
                    value.controls = Some(SemanticId::new("controlled").unwrap());
                    value.busy = Some(true);
                    value.environment = Some(Environment {
                        direction: Direction::Rtl,
                        language: LanguageTag::new("ar").unwrap(),
                    });
                }
                2 => {
                    value.navigation = Some(Orientation::Horizontal);
                }
                3 => {
                    value.navigation = Some(Orientation::Vertical);
                }
                4 => {
                    value.id = Some(SemanticId::new("layer").unwrap());
                    value.layer = Some(Layer {
                        initial_focus: Some(SemanticId::new("cancel").unwrap()),
                        ..Default::default()
                    });
                    base.focus = FocusMode::Programmatic;
                }
                5 => {
                    value.grid_multiple = true;
                }
                6 => {
                    value.listbox_multiple = true;
                }
                _ => {}
            }
            if metadata != 0 {
                base.semantics = Some(Box::new(value));
            }
            if shape(&base).is_ok() {
                bases.push(base);
            }
        }
    }
    for props in [
        Props::Text(TextProps::default()),
        Props::Input(InputProps::default()),
        Props::Button(ButtonProps::default()),
        Props::Group,
    ] {
        bases.push(Descriptor::new(props));
    }
    let mut changes: Vec<_> = roles().into_iter().map(Change::Set).collect();
    changes.extend([Change::Keep, Change::Clear]);
    let (mut accepted, mut rejected) = (0, 0);
    for base in bases {
        for change in &changes {
            let patch = Patch::Role(change.clone());
            let expected = base.patched(patch.clone()).and_then(|next| {
                crate::difference(&shape(&base)?, &shape(&next)?, false, false, false)
            });
            for planned in [false, true] {
                let runtime = RuntimeId::new();
                let id = NodeIds::new(runtime).allocate();
                let mut mapper = Mapper::new(runtime);
                let mut sink = Record::default();
                let create = if planned {
                    Mutation::CreatePlan {
                        plan: Plan::new(vec![PlanNode {
                            parent: None,
                            descriptor: base.clone(),
                        }])
                        .unwrap(),
                        ids: vec![id],
                    }
                } else {
                    Mutation::Create {
                        id,
                        node: Box::new(base.clone()),
                    }
                };
                mapper
                    .apply(
                        &[
                            create,
                            Mutation::Place {
                                id,
                                parent: None,
                                before: None,
                            },
                        ],
                        &mut sink,
                    )
                    .unwrap();
                let before = mapper.receipt();
                let stats = mapper.tree().stats();
                let ops = sink.0.len();
                let result = mapper.apply(
                    &[Mutation::Patch {
                        id,
                        patch: patch.clone(),
                    }],
                    &mut sink,
                );
                match &expected {
                    Ok(edits) => {
                        accepted += 1;
                        assert_eq!(result.unwrap().applied, before.applied + 1);
                        let Some(Op::Patch { edits: actual, .. }) = sink.0.last() else {
                            panic!("missing patch")
                        };
                        assert_eq!(actual, edits, "{base:?}/{patch:?}");
                        assert_eq!(
                            mapper.tree().node(id).unwrap().descriptor,
                            base.patched(patch.clone()).unwrap()
                        );
                    }
                    Err(_) => {
                        rejected += 1;
                        assert!(result.is_err(), "{base:?}/{patch:?}");
                        assert_eq!(mapper.receipt(), before);
                        assert_eq!(mapper.tree().stats(), stats);
                        assert_eq!(mapper.tree().node(id).unwrap().descriptor, base);
                        assert_eq!(sink.0.len(), ops);
                    }
                }
            }
        }
    }
    assert!(accepted > 1000 && rejected > 1000);
    println!("role full-shape oracle: accepted={accepted}, rejected={rejected}");
}
