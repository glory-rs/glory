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
fn oracle(base: &Descriptor, patch: Patch) {
    let next = base.patched(patch.clone()).unwrap();
    let expected = crate::difference(
        &shape(base).unwrap(),
        &shape(&next).unwrap(),
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
        .apply(&[Mutation::Patch { id, patch }], &mut sink)
        .unwrap();
    assert!(matches!(&sink.0[0],Op::Patch{edits,..}if edits==&expected));
}
#[test]
fn menubar_intrinsic_refinement_role_focus_and_defaults_match_full_shape_oracle() {
    let mut bar = Descriptor::new(Props::View);
    bar.name = Some("Application".into());
    bar.role = Some(Role::Menubar);
    bar.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("app-bar").unwrap()),
        navigation: Some(Orientation::Horizontal),
        ..Default::default()
    }));
    let attrs = shape(&bar)
        .unwrap()
        .pieces
        .into_iter()
        .find_map(|piece| match piece.kind {
            Kind::Element { attributes, .. } => Some(attributes),
            _ => None,
        })
        .unwrap();
    assert_eq!(attrs.get("role").map(String::as_str), Some("menubar"));
    assert_eq!(
        attrs.get("aria-orientation").map(String::as_str),
        Some("horizontal")
    );
    for patch in [
        Patch::Role(Change::Keep),
        Patch::Role(Change::Set(Some(Role::Group))),
        Patch::Role(Change::Clear),
    ] {
        oracle(&bar, patch);
    }
    let mut root = Descriptor::new(Props::Button(ButtonProps {
        label: "File".into(),
        ..Default::default()
    }));
    root.focus = FocusMode::TabStop;
    root.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("file-root").unwrap()),
        menubar_item: true,
        navigation_item: true,
        popup: Some(PopupKind::Menu),
        expanded: Some(false),
        controls: Some(SemanticId::new("file-menu").unwrap()),
        menu_item: Some(MenuItemSemantics {
            kind: MenuItemKind::Action,
            owner: SemanticId::new("app-bar").unwrap(),
        }),
        ..Default::default()
    }));
    let mut ordinary = root.semantics.clone().unwrap();
    ordinary.menubar_item = false;
    for patch in [
        Patch::Semantics(Change::Keep),
        Patch::Semantics(Change::Set(Some(ordinary))),
        Patch::Semantics(Change::Clear),
        Patch::Focus(Change::Set(FocusMode::Programmatic)),
    ] {
        oracle(&root, patch);
    }
    let shape = shape(&root).unwrap();
    let attrs = shape
        .pieces
        .iter()
        .find_map(|piece| match &piece.kind {
            Kind::Element { attributes, .. } => Some(attributes),
            _ => None,
        })
        .unwrap();
    assert_eq!(attrs.get("role").map(String::as_str), Some("menuitem"));
    assert_eq!(attrs.get("aria-haspopup").map(String::as_str), Some("menu"));
    assert!(attrs.contains_key("data-gly-menubar-item"));
}
