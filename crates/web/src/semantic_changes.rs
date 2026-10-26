//! One borrowed view of the batch's final committed metadata. This is not a
//! node/visit cache: it lives only through one adapter update and stores no
//! owned business payloads. Live feature membership remains owned by each driver.
use glory_render::{Mutation, NodeId, Patch, Semantics, memory::Memory};
use std::collections::BTreeMap;

pub(crate) type Changes<'a> = BTreeMap<NodeId, Option<&'a Semantics>>;

/// Potential effects of this one submitted batch, not a second live-node index.
/// A rejected suffix may over-invalidate; its payload is never installed here.
#[derive(Default)]
pub(crate) struct Batch<'a> {
    pub metadata: Changes<'a>,
    pub state: StateChanges,
}

#[derive(Default)]
pub(crate) struct StateChanges {
    pub structure: bool,
    pub eligibility: bool,
    pub focus: bool,
    pub rich_document: bool,
    pub file_configuration: bool,
}
impl StateChanges {
    pub fn inherited(&self) -> bool {
        self.structure || self.eligibility
    }

    fn record(&mut self, mutation: &Mutation) {
        match mutation {
            Mutation::Create { .. }
            | Mutation::CreatePlan { .. }
            | Mutation::Place { .. }
            | Mutation::Resume { .. }
            | Mutation::Suspend { .. }
            | Mutation::Dispose { .. } => self.structure = true,
            Mutation::Patch { patch, .. } => match patch {
                // Role participates in inherited enabled state (for example a
                // disabled Tree/Menubar). Metadata includes hidden/inert/readonly.
                Patch::Enabled(_) | Patch::Role(_) | Patch::Semantics(_) => self.eligibility = true,
                Patch::Focus(_) => self.focus = true,
                Patch::RichDocument { .. } => self.rich_document = true,
                Patch::File { .. } => self.file_configuration = true,
                Patch::Text(_)
                | Patch::TextKind(_)
                | Patch::Selectable(_)
                | Patch::Label(_)
                | Patch::Value { .. }
                | Patch::SelectSelection { .. }
                | Patch::Purpose(_)
                | Patch::Multiline(_)
                | Patch::Checked(_)
                | Patch::CheckMode(_)
                | Patch::Source { .. }
                | Patch::Alternative(_)
                | Patch::Graphic(_)
                | Patch::Target(_)
                | Patch::Classes(_)
                | Patch::Style(_)
                | Patch::Name(_)
                | Patch::Item(_) => {}
            },
        }
    }
}

/// Resolve only metadata-bearing creations and explicit metadata patches.
/// Read the accepted tree, not payloads from a rejected suffix. Keep None for
/// Clear so existing controllers can retire. Retired generations are omitted.
pub(crate) fn collect<'a>(tree: &'a Memory, mutations: &[Mutation]) -> Batch<'a> {
    let mut changed = BTreeMap::new();
    let mut state = StateChanges::default();
    let mut record = |id| {
        if let Ok(node) = tree.node(id) {
            changed.insert(id, node.descriptor.view().semantics.as_deref());
        }
    };
    for mutation in mutations {
        state.record(mutation);
        match mutation {
            Mutation::Create { id, node } if node.semantics.is_some() => record(*id),
            Mutation::CreatePlan { plan, ids } => {
                for &index in plan.semantic_nodes() {
                    // A rejected suffix may contain a malformed identity list.
                    if let Some(&id) = ids.get(index) {
                        record(id);
                    }
                }
            }
            Mutation::Patch {
                id,
                patch: Patch::Semantics(_),
            } => record(*id),
            _ => {}
        }
    }
    Batch {
        metadata: changed,
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_render::*;

    fn descriptor(metadata: bool) -> Descriptor {
        let mut node = Descriptor::new(Props::View);
        if metadata {
            node.semantics = Some(Box::new(Semantics {
                busy: Some(true),
                ..Default::default()
            }));
        }
        node
    }
    fn legacy<'a>(tree: &'a Memory, mutations: &[Mutation]) -> Changes<'a> {
        mutations
            .iter()
            .flat_map(|mutation| match mutation {
                Mutation::Create { id, .. }
                | Mutation::Patch {
                    id,
                    patch: Patch::Semantics(_),
                } => std::slice::from_ref(id),
                Mutation::CreatePlan { ids, .. } => ids.as_slice(),
                _ => &[],
            })
            .filter_map(|id| {
                tree.node(*id)
                    .ok()
                    .map(|node| (*id, node.descriptor.view().semantics.as_deref()))
            })
            .collect()
    }
    fn replay(
        tree: &Memory,
        mut live: BTreeMap<NodeId, Semantics>,
        changes: Changes<'_>,
    ) -> BTreeMap<NodeId, Semantics> {
        for (id, value) in changes {
            if let Some(value) = value {
                live.insert(id, value.clone());
            } else {
                live.remove(&id);
            }
        }
        live.retain(|id, _| tree.node(*id).is_ok());
        live
    }
    #[test]
    fn ordinary_plans_do_not_build_a_per_instance_metadata_inventory() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let plan = Plan::new(
            (0..MAX_PLAN_NODES)
                .map(|index| PlanNode {
                    parent: (index != 0).then_some(0),
                    descriptor: descriptor(false),
                })
                .collect(),
        )
        .unwrap();
        assert!(plan.semantic_nodes().is_empty());
        let commands: Vec<_> = (0..64)
            .map(|_| Mutation::CreatePlan {
                plan: plan.clone(),
                ids: plan.nodes().iter().map(|_| ids.allocate()).collect(),
            })
            .collect();
        let mut tree = Memory::new(runtime);
        tree.apply(&commands).unwrap();
        assert_eq!(legacy(&tree, &commands).len(), 64 * MAX_PLAN_NODES);
        assert!(collect(&tree, &commands).metadata.is_empty());
    }
    #[test]
    fn final_borrowed_metadata_and_clear_match_the_old_driver_replay() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let plan = Plan::new(vec![
            PlanNode {
                parent: None,
                descriptor: descriptor(false),
            },
            PlanNode {
                parent: Some(0),
                descriptor: descriptor(true),
            },
        ])
        .unwrap();
        assert_eq!(plan.semantic_nodes(), &[1]);
        let (root, child, ordinary, retired) = (
            ids.allocate(),
            ids.allocate(),
            ids.allocate(),
            ids.allocate(),
        );
        let mut tree = Memory::new(runtime);
        let commands = vec![
            Mutation::CreatePlan {
                plan,
                ids: vec![root, child],
            },
            Mutation::Patch {
                id: root,
                patch: Patch::Semantics(Change::Set(descriptor(true).semantics)),
            },
            Mutation::Patch {
                id: child,
                patch: Patch::Semantics(Change::Clear),
            },
            Mutation::Create {
                id: ordinary,
                node: Box::new(descriptor(false)),
            },
            Mutation::Create {
                id: retired,
                node: Box::new(descriptor(true)),
            },
            Mutation::Dispose { id: retired },
            Mutation::Patch {
                id: root,
                patch: Patch::Semantics(Change::Keep),
            },
        ];
        tree.apply(&commands).unwrap();
        let changed = collect(&tree, &commands).metadata;
        assert_eq!(changed.len(), 2);
        assert!(changed[&child].is_none());
        assert!(std::ptr::eq(
            changed[&root].unwrap(),
            tree.node(root)
                .unwrap()
                .descriptor
                .view()
                .semantics
                .as_deref()
                .unwrap()
        ));
        let live = BTreeMap::from([
            (child, Semantics::default()),
            (retired, Semantics::default()),
        ]);
        assert_eq!(
            replay(&tree, live.clone(), legacy(&tree, &commands)),
            replay(&tree, live, changed)
        );
    }
    #[test]
    fn rejected_suffix_and_recycled_slots_read_only_the_accepted_generation() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let old = ids.allocate();
        let mut tree = Memory::new(runtime);
        tree.apply(&[
            Mutation::Create {
                id: old,
                node: Box::new(descriptor(true)),
            },
            Mutation::Dispose { id: old },
        ])
        .unwrap();
        ids.release(old);
        let current = ids.allocate();
        let plan = Plan::new(vec![PlanNode {
            parent: None,
            descriptor: descriptor(true),
        }])
        .unwrap();
        let commands = vec![
            Mutation::Create {
                id: current,
                node: Box::new(descriptor(true)),
            },
            Mutation::CreatePlan { plan, ids: vec![] },
            Mutation::Patch {
                id: old,
                patch: Patch::Semantics(Change::Clear),
            },
        ];
        assert!(tree.apply(&commands).is_err());
        let receipt = tree.receipt();
        let changed = collect(&tree, &commands).metadata;
        assert_eq!(changed.len(), 1);
        assert!(changed[&current].is_some());
        assert!(!changed.contains_key(&old));
        assert_eq!(tree.receipt(), receipt);
        assert_eq!(
            replay(&tree, BTreeMap::new(), changed),
            replay(&tree, BTreeMap::new(), legacy(&tree, &commands))
        );
    }

    #[test]
    fn control_invalidations_cover_actual_projection_changes_and_skip_local_patches() {
        fn rich(
            tree: &Memory,
            id: NodeId,
        ) -> Option<(bool, bool, bool, bool, FocusMode, bool, RichEditorProps)> {
            let node = tree.node(id).ok()?;
            let d = node.descriptor.view();
            let Props::RichEditor(props) = d.props else {
                panic!()
            };
            Some((
                tree.active(id),
                tree.enabled(id),
                tree.inert(id),
                d.readonly(),
                d.focus,
                d.semantics.as_ref().is_some_and(|s| s.disabled),
                props.clone(),
            ))
        }
        fn scopes(tree: &Memory) -> Vec<(NodeId, FocusScopeConfig)> {
            tree.focus_scope_roots()
                .filter(|&id| tree.active(id) && tree.enabled(id) && !tree.inert(id))
                .map(|id| {
                    (
                        id,
                        tree.node(id)
                            .unwrap()
                            .descriptor
                            .view()
                            .semantics
                            .as_ref()
                            .unwrap()
                            .focus_scope
                            .clone()
                            .unwrap(),
                    )
                })
                .collect()
        }
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let (parent, editor, file, text) = (
            ids.allocate(),
            ids.allocate(),
            ids.allocate(),
            ids.allocate(),
        );
        let mut tree = Memory::new(runtime);
        let metadata = |inert, readonly| {
            Some(Box::new(Semantics {
                inert,
                readonly,
                focus_scope: Some(FocusScopeConfig {
                    auto_focus: false,
                    ..Default::default()
                }),
                ..Default::default()
            }))
        };
        let mut root = Descriptor::new(Props::View);
        root.focus = FocusMode::Programmatic;
        root.semantics = metadata(false, false);
        let mut initial = Vec::new();
        for (id, descriptor) in [
            (parent, root),
            (
                editor,
                Descriptor::new(Props::RichEditor(Default::default())),
            ),
            (file, Descriptor::new(Props::File(Default::default()))),
            (text, Descriptor::new(Props::Text(Default::default()))),
        ] {
            initial.push(Mutation::Create {
                id,
                node: Box::new(descriptor),
            });
            initial.push(Mutation::Place {
                id,
                parent: (id != parent).then_some(parent),
                before: None,
            });
        }
        tree.apply(&initial).unwrap();
        let mut commands = vec![
            Mutation::Patch {
                id: text,
                patch: Patch::Text(Change::Set("local text".into())),
            },
            Mutation::Patch {
                id: text,
                patch: Patch::TextKind(Change::Set(TextKind::Paragraph)),
            },
            Mutation::Patch {
                id: parent,
                patch: Patch::Classes(Change::Set(vec!["local".into()])),
            },
            Mutation::Patch {
                id: parent,
                patch: Patch::Style(Change::Set(Box::new(
                    style::Style::new().width(style::Length::Px(100.)),
                ))),
            },
            Mutation::Patch {
                id: editor,
                patch: Patch::Name(Change::Set(Some("editor".into()))),
            },
            Mutation::Patch {
                id: editor,
                patch: Patch::Focus(Change::Set(FocusMode::Programmatic)),
            },
            Mutation::Patch {
                id: editor,
                patch: Patch::Enabled(Change::Set(false)),
            },
            Mutation::Patch {
                id: editor,
                patch: Patch::Enabled(Change::Clear),
            },
            Mutation::Patch {
                id: parent,
                patch: Patch::Semantics(Change::Set(metadata(true, false))),
            },
            Mutation::Patch {
                id: parent,
                patch: Patch::Semantics(Change::Set(metadata(false, false))),
            },
            Mutation::Patch {
                id: editor,
                patch: Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                    readonly: true,
                    ..Default::default()
                })))),
            },
            Mutation::Patch {
                id: editor,
                patch: Patch::RichDocument {
                    document: Change::Set(std::rc::Rc::new(RichDocument {
                        blocks: vec![RichBlock {
                            runs: vec![RichRun {
                                text: "changed".into(),
                                ..Default::default()
                            }],
                            ..Default::default()
                        }],
                    })),
                    revision: 1,
                },
            },
            Mutation::Patch {
                id: file,
                patch: Patch::File {
                    configuration: Change::Set(Box::default()),
                    revision: 1,
                },
            },
            Mutation::Suspend { id: parent },
            Mutation::Resume {
                id: parent,
                parent: None,
                before: None,
            },
        ];
        commands.push(Mutation::Dispose { id: parent });
        let mut skipped = 0;
        for command in commands {
            let old_rich = rich(&tree, editor);
            let old_scopes = scopes(&tree);
            let old_file = tree
                .node(file)
                .ok()
                .map(|node| node.descriptor.view().props.clone());
            tree.apply(std::slice::from_ref(&command)).unwrap();
            let changes = collect(&tree, std::slice::from_ref(&command));
            let state = changes.state;
            if rich(&tree, editor) != old_rich {
                assert!(
                    state.inherited() || state.focus || state.rich_document,
                    "{command:?}"
                );
            }
            if scopes(&tree) != old_scopes {
                assert!(state.inherited(), "{command:?}");
            }
            if tree
                .node(file)
                .ok()
                .map(|node| node.descriptor.view().props.clone())
                != old_file
            {
                assert!(state.structure || state.file_configuration, "{command:?}");
            }
            if !(state.inherited()
                || state.focus
                || state.rich_document
                || state.file_configuration)
            {
                skipped += 1;
            }
        }
        assert_eq!(
            skipped, 5,
            "local content, layout and name leave control projections stable"
        );
    }

    #[test]
    fn rejected_role_suffix_preserves_the_accepted_control_projection() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let (parent, child) = (ids.allocate(), ids.allocate());
        let mut tree = Memory::new(runtime);
        let mut descriptor = Descriptor::new(Props::View);
        descriptor.role = Some(Role::Tree);
        descriptor.semantics = Some(Box::new(Semantics {
            disabled: true,
            ..Default::default()
        }));
        tree.apply(&[
            Mutation::Create {
                id: parent,
                node: Box::new(descriptor),
            },
            Mutation::Place {
                id: parent,
                parent: None,
                before: None,
            },
            Mutation::Create {
                id: child,
                node: Box::new(Descriptor::new(Props::RichEditor(Default::default()))),
            },
            Mutation::Place {
                id: child,
                parent: Some(parent),
                before: None,
            },
        ])
        .unwrap();
        assert!(!tree.enabled(child));
        let accepted = tree.receipt().applied;
        let commands = [
            Mutation::Patch {
                id: child,
                patch: Patch::Name(Change::Set(Some("accepted".into()))),
            },
            Mutation::Patch {
                id: parent,
                patch: Patch::Role(Change::Clear),
            },
        ];
        assert!(tree.apply(&commands).is_err());
        assert_eq!(tree.receipt().applied, accepted + 1);
        assert_eq!(
            tree.node(child).unwrap().descriptor.view().name.as_deref(),
            Some("accepted")
        );
        assert_eq!(
            tree.node(parent).unwrap().descriptor.view().role,
            Some(Role::Tree)
        );
        assert!(!tree.enabled(child));
        let changes = collect(&tree, &commands);
        assert!(changes.metadata.is_empty());
        assert!(
            changes.state.inherited(),
            "rejected suffix can only over-invalidate"
        );
        assert_eq!(tree.receipt().applied, accepted + 1);
    }
}
