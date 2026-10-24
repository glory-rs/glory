#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;

#[test]
fn native_combobox_option_refinements_reject_before_facets_with_prefix_watermarks() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let second = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let plain = Descriptor::new(Props::Button(ButtonProps::default()));
    let mut combo = plain.clone();
    combo.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("combo").unwrap()),
        combobox: Some(ComboboxKind::Select),
        controls: Some(SemanticId::new("box").unwrap()),
        popup: Some(PopupKind::Listbox),
        expanded: Some(false),
        ..Default::default()
    }));
    let patch = Patch::Semantics(Change::Set(combo.semantics.clone()));
    let mut editable = Descriptor::new(Props::Input(InputProps::default()));
    editable.semantics = combo.semantics.clone();
    editable.semantics.as_mut().unwrap().combobox = Some(ComboboxKind::Editable);
    assert_eq!(
        renderer.preflight(editable.view(), None),
        Err(Error::Unsupported("native combobox controls"))
    );
    assert_eq!(
        renderer.preflight(combo.view(), None),
        Err(Error::Unsupported("native combobox controls"))
    );
    assert_eq!(
        renderer.preflight(plain.view(), Some(&patch)),
        Err(Error::Unsupported("native combobox controls"))
    );
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: first,
                node: Box::new(plain)
            },
            Mutation::Place {
                id: first,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: second,
                node: Box::new(combo)
            }
        ]),
        Err(Error::Unsupported("native combobox controls"))
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    assert!(renderer.engine_id(second, glory_html::Part::Main).is_err());
    for kind in [ComboboxKind::Autocomplete, ComboboxKind::Multiple] {
        let mut refined = Descriptor::new(Props::Input(InputProps::default()));
        refined.semantics = Some(Box::new(Semantics {
            id: Some(SemanticId::new("extra-editor").unwrap()),
            combobox: Some(kind),
            controls: Some(SemanticId::new("later-listbox").unwrap()),
            popup: Some(PopupKind::Listbox),
            expanded: Some(false),
            ..Default::default()
        }));
        assert_eq!(
            renderer.preflight(refined.view(), None),
            Err(Error::Unsupported("native combobox controls"))
        );
        let input = ids.allocate();
        assert_eq!(
            renderer.apply(&[Mutation::Create {
                id: input,
                node: Box::new(refined)
            }]),
            Err(Error::Unsupported("native combobox controls"))
        );
        assert!(renderer.engine_id(input, glory_html::Part::Main).is_err());
        assert_eq!(renderer.stats().applied, 2);
    }
    assert_eq!(
        renderer.apply(&[Mutation::Create {
            id: second,
            node: Box::new(editable)
        }]),
        Err(Error::Unsupported("native combobox controls"))
    );
    assert!(renderer.engine_id(second, glory_html::Part::Main).is_err());
    assert_eq!(
        renderer.apply(&[Mutation::Patch { id: first, patch }]),
        Err(Error::Unsupported("native combobox controls"))
    );
    let mut option = Descriptor::new(Props::Button(ButtonProps::default()));
    option.focus = FocusMode::Programmatic;
    option.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("option").unwrap()),
        option: Some(OptionSemantics {
            owner: SemanticId::new("box").unwrap(),
        }),
        selected: Some(false),
        ..Default::default()
    }));
    assert_eq!(
        renderer.preflight(option.view(), None),
        Err(Error::Unsupported("native combobox controls"))
    );
    renderer
        .request(Query::Bounds(first), QueryToken(921))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(921),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
