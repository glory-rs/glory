//! Browser probe for control dependencies and Owner-preserving placement.
use glory::prelude::*;
use glory::render::{
    Change, Descriptor, FileOutcome, FileRead, FileToken, NodeId, Patch, Props, Query, QueryValue,
    RichBlock, RichDocument, RichRun,
};
use glory::ui::Mounted;
use glory::web::WebApp;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::prelude::*;

type Result<T = ()> = std::result::Result<T, JsValue>;
struct Probe {
    web: WebApp,
    mounted: Mounted,
    _scopes: Vec<Scope>,
    fieldset: NodeId,
    editor: NodeId,
    file: NodeId,
    destination: NodeId,
    other: NodeId,
    ends: Rc<Cell<usize>>,
    files: Rc<RefCell<Vec<FileToken>>>,
    read: Rc<RefCell<String>>,
}
thread_local! { static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) }; }
pub(crate) fn clear() {
    drop(PROBE.with(|probe| probe.borrow_mut().take()));
}
fn js(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
fn fieldset(inert: bool, disabled: bool) -> Semantics {
    Semantics {
        inert,
        disabled,
        field_part: Some(glory::render::FieldPart::Fieldset),
        ..Default::default()
    }
}
fn create(scope: &Scope, props: Props, class: &str) -> glory::ui::Result<NodeId> {
    let mut descriptor = Descriptor::new(props);
    descriptor.classes.push(class.into());
    scope.create(descriptor)
}

#[wasm_bindgen]
pub fn controller_probe(command: u8) -> Result<String> {
    if command == 0 {
        crate::browser::unmount();
        let root = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("app")
            .unwrap();
        let web = glory::web::launch(root, group()).map_err(js)?;
        let output = Rc::new(RefCell::new(None));
        let out = output.clone();
        let ends = Rc::new(Cell::new(0));
        let ended = ends.clone();
        let files = Rc::new(RefCell::new(Vec::new()));
        let selected = files.clone();
        let mounted = widget(move |scope| {
            let mut descriptor = Descriptor::new(Props::View);
            descriptor.focus = FocusMode::Programmatic;
            descriptor.semantics = Some(Box::new(Semantics {
                focus_scope: Some(glory::render::FocusScopeConfig {
                    auto_focus: false,
                    ..Default::default()
                }),
                ..Default::default()
            }));
            let root = scope.create(descriptor)?;
            let parent = scope.child_in(root)?;
            let mut descriptor = Descriptor::new(Props::View);
            descriptor.classes.push("controller-fieldset".into());
            descriptor.semantics = Some(Box::new(fieldset(false, false)));
            let field = parent.create(descriptor)?;
            let children = parent.child_in(field)?;
            let editor = create(
                &children,
                Props::RichEditor(Default::default()),
                "controller-editor",
            )?;
            let file = create(
                &children,
                Props::File(Default::default()),
                "controller-file",
            )?;
            let ended = ended.clone();
            children.on(
                editor,
                glory::render::EventKind::Composition,
                move |event| {
                    if let glory::render::Event::Composition { active: false } = event {
                        ended.set(ended.get() + 1);
                    }
                },
            )?;
            let selected = selected.clone();
            children.on(file, glory::render::EventKind::Files, move |event| {
                if let glory::render::Event::Files(event) = event
                    && let FileOutcome::Selected(files) = event.outcome
                {
                    *selected.borrow_mut() = files.into_iter().map(|file| file.token).collect();
                }
            })?;
            *out.borrow_mut() = Some((parent, children, field, editor, file));
            Ok(())
        })
        .mount(web.app().scope())
        .map_err(js)?;
        let (parent, children, fieldset, editor, file) = output.borrow_mut().take().unwrap();
        let destination =
            create(web.app().scope(), Props::View, "controller-destination").map_err(js)?;
        let other = create(
            web.app().scope(),
            Props::Button(Default::default()),
            "controller-other",
        )
        .map_err(js)?;
        web.app().flush().map_err(js)?;
        PROBE.with(|probe| {
            *probe.borrow_mut() = Some(Probe {
                web,
                mounted,
                _scopes: vec![parent, children],
                fieldset,
                editor,
                file,
                destination,
                other,
                ends,
                files,
                read: Rc::new(RefCell::new("none".into())),
            })
        });
    } else if command == 255 {
        clear();
        return Ok("null".into());
    }
    PROBE.with(|probe| {
        let probe = probe.borrow();
        let probe = probe
            .as_ref()
            .ok_or_else(|| js("missing controller probe"))?;
        let app = probe.web.app();
        let scope = app.scope();
        let patch = |node, patch| scope.patch(node, patch).map_err(js);
        match command {
            0 => {}
            1 => {
                patch(
                    probe.other,
                    Patch::Label(Change::Set("unrelated label".into())),
                )?;
                patch(
                    probe.other,
                    Patch::Classes(Change::Set(vec![
                        "controller-other".into(),
                        "changed".into(),
                    ])),
                )?;
            }
            2..=5 => patch(
                probe.fieldset,
                Patch::Semantics(Change::Set(Some(Box::new(fieldset(
                    command == 2,
                    command == 4,
                ))))),
            )?,
            6 => patch(
                probe.editor,
                Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                    readonly: true,
                    ..Default::default()
                })))),
            )?,
            7 => patch(probe.editor, Patch::Semantics(Change::Clear))?,
            8 => probe.mounted.suspend().map_err(js)?,
            9 => probe.mounted.place_before(None).map_err(js)?,
            10 => patch(
                probe.editor,
                Patch::Focus(Change::Set(FocusMode::Programmatic)),
            )?,
            11 => patch(probe.editor, Patch::Focus(Change::Clear))?,
            12 => patch(
                probe.file,
                Patch::File {
                    configuration: Change::Set(Box::default()),
                    revision: 1,
                },
            )?,
            13 => {
                patch(
                    probe.destination,
                    Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                        inert: true,
                        ..Default::default()
                    })))),
                )?;
                scope
                    .place(probe.editor, Some(probe.destination), None)
                    .map_err(js)?;
            }
            14 => scope
                .place(probe.editor, Some(probe.fieldset), None)
                .map_err(js)?,
            15 => patch(
                probe.editor,
                Patch::RichDocument {
                    document: Change::Set(Rc::new(RichDocument {
                        blocks: vec![RichBlock {
                            runs: vec![RichRun {
                                text: "program 😀".into(),
                                ..Default::default()
                            }],
                            ..Default::default()
                        }],
                    })),
                    revision: 1,
                },
            )?,
            16 => {
                patch(
                    probe.destination,
                    Patch::Role(Change::Set(Some(Role::Tree))),
                )?;
                patch(
                    probe.destination,
                    Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                        disabled: true,
                        ..Default::default()
                    })))),
                )?;
                scope
                    .place(probe.editor, Some(probe.destination), None)
                    .map_err(js)?;
            }
            17 => patch(
                probe.destination,
                Patch::Role(Change::Set(Some(Role::TreeItem))),
            )?,
            18 => {
                patch(probe.destination, Patch::Semantics(Change::Clear))?;
                patch(probe.destination, Patch::Role(Change::Clear))?;
            }
            19 => {
                let file = *probe
                    .files
                    .borrow()
                    .first()
                    .ok_or_else(|| js("select a file first"))?;
                *probe.read.borrow_mut() = "pending".into();
                let out = probe.read.clone();
                scope
                    .request(
                        Query::FileRead(FileRead {
                            file,
                            offset: 0,
                            length: 3,
                        }),
                        move |reply| {
                            *out.borrow_mut() = match reply {
                                Ok(QueryValue::FileBytes(bytes)) => format!("{:?}", bytes.bytes),
                                other => format!("{other:?}"),
                            };
                        },
                    )
                    .map_err(js)?;
            }
            20 => {}
            _ => return Err(js("unknown controller command")),
        }
        app.flush().map_err(js)?;
        app.pump().map_err(js)?;
        let files = probe.files.borrow();
        let valid = files
            .iter()
            .filter(|file| app.tree().selected_file(**file).is_ok())
            .count();
        let read = js_sys::JSON::stringify(&JsValue::from_str(&probe.read.borrow()))?;
        Ok(format!(
            "{{\"ends\":{},\"files\":{},\"validFiles\":{},\"read\":{},\"nodes\":{}}}",
            probe.ends.get(),
            files.len(),
            valid,
            read.as_string().unwrap(),
            app.stats().nodes
        ))
    })
}
