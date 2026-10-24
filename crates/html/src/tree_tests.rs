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
#[test]
fn tree_metadata_and_each_reset_match_full_shape_without_replacing_physical_nodes() {
    for role in [Role::Tree, Role::TreeItem] {
        let metadata = if role == Role::Tree {
            Semantics {
                tree_navigation: Some(TreeNavigation::default()),
                ..Default::default()
            }
        } else {
            Semantics {
                tree_item: Some(TreeItem {
                    level: 2,
                    position: 1,
                    count: 3,
                }),
                expanded: Some(true),
                selected: Some(true),
                disabled: true,
                ..Default::default()
            }
        };
        let mut base = Descriptor::new(Props::View);
        base.role = Some(role);
        base.semantics = Some(Box::new(metadata));
        let mut changed = base.clone();
        let s = changed.semantics.as_mut().unwrap();
        if role == Role::Tree {
            s.tree_navigation = Some(TreeNavigation {
                direction: Direction::Rtl,
                typeahead: false,
                multiple: true,
            });
        } else {
            s.tree_item = Some(TreeItem {
                level: 3,
                position: 0,
                count: 2,
            });
            s.expanded = Some(false);
            s.selected = Some(false);
            s.disabled = false;
        }
        for next in [changed, {
            let mut clear = base.clone();
            clear.semantics = None;
            clear
        }] {
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
                .apply(
                    &[Mutation::Patch {
                        id,
                        patch: Patch::Semantics(Change::Set(next.semantics)),
                    }],
                    &mut sink,
                )
                .unwrap();
            assert!(matches!(&sink.0[0],Op::Patch{edits,..} if edits==&expected));
        }
        let attrs = crate::semantics::attributes(base.semantics.as_deref(), base.role);
        if role == Role::TreeItem {
            assert_eq!(attrs.get("aria-level").map(String::as_str), Some("2"));
            assert_eq!(attrs.get("aria-posinset").map(String::as_str), Some("2"));
            assert_eq!(attrs.get("aria-setsize").map(String::as_str), Some("3"));
        }
    }
}
#[test]
fn checked_tree_fields_and_clear_match_the_full_shape_oracle() {
    for checked in [
        TreeCheckState::Unchecked,
        TreeCheckState::Checked,
        TreeCheckState::Mixed,
    ] {
        let mut base = Descriptor::new(Props::View);
        base.role = Some(Role::TreeItem);
        base.semantics = Some(Box::new(Semantics {
            tree_item: Some(TreeItem {
                level: 1,
                position: 0,
                count: 1,
            }),
            tree_checked: Some(checked),
            ..Default::default()
        }));
        assert_eq!(
            crate::semantics::attributes(base.semantics.as_deref(), base.role)["aria-checked"],
            checked.as_str()
        );
        for next in [
            None,
            Some(Box::new(Semantics {
                tree_item: Some(TreeItem {
                    level: 1,
                    position: 0,
                    count: 1,
                }),
                ..Default::default()
            })),
        ] {
            let mut changed = base.clone();
            changed.semantics = next.clone();
            let expected = crate::difference(
                &shape(&base).unwrap(),
                &shape(&changed).unwrap(),
                false,
                false,
                false,
            )
            .unwrap();
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
                .apply(
                    &[Mutation::Patch {
                        id,
                        patch: Patch::Semantics(Change::Set(next)),
                    }],
                    &mut sink,
                )
                .unwrap();
            assert!(matches!(&sink.0[0],Op::Patch{edits,..} if edits==&expected));
        }
    }
}
