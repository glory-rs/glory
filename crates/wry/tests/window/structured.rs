use glory_render::{
    Event, EventKind, FileConfig, FileOutcome, FilePreviewLimits, FileRead, InputEdit,
    InputEditCapture, Query, QueryValue, Semantics,
};
use glory_ui::{button, file, group, input, rich_editor, text, view};
use glory_wry::shell::WindowApp;

pub(super) fn mount(app: &WindowApp) -> glory_ui::Result {
    app.renderer().enable_files()?;
    let scope = app.app().scope();
    let form_state = scope.cage(String::new());
    let submit = scope.cage(None);
    let file_state = scope.cage(String::new());
    let header_state = scope.cage(String::new());
    let edit_state = scope.cage(String::new());
    let rich_state = scope.cage(String::new());
    let node = scope.cage(None);
    let file_scope = scope.downgrade();
    let rich_scope = scope.downgrade();
    app.mount(
        view()
            .class("structured-form")
            .semantics(Some(Box::new(Semantics {
                field_part: Some(glory_render::FieldPart::Form),
                ..Default::default()
            })))
            .on(EventKind::Submit, move |event| {
                let Event::Submit { submitter } = event else {
                    return;
                };
                if let Some(actual) = submitter {
                    assert_eq!(Some(actual), submit.get());
                }
                form_state.revise(|v| v.push('S'));
            })
            .on(EventKind::Reset, move |_| {
                form_state.revise(|v| v.push('R'))
            })
            .fill((
                input().class("structured-form-input"),
                button()
                    .class("structured-submit")
                    .node_ref(submit)
                    .label("Submit")
                    .semantics(Some(Box::new(Semantics {
                        form_action: Some(glory_render::FormAction::Submit),
                        ..Default::default()
                    }))),
                button()
                    .class("structured-reset")
                    .label("Reset")
                    .semantics(Some(Box::new(Semantics {
                        form_action: Some(glory_render::FormAction::Reset),
                        ..Default::default()
                    }))),
                text().class("structured-form-state").content(form_state),
            )),
    )?;
    app.mount(
        group().fill((
            file()
                .class("structured-file")
                .configuration(FileConfig {
                    drop: true,
                    preview: Some(FilePreviewLimits::default()),
                    ..Default::default()
                })
                .on(EventKind::Files, move |event| {
                    let Event::Files(event) = event else { return };
                    let FileOutcome::Selected(files) = event.outcome else {
                        return;
                    };
                    let scope = file_scope.upgrade().unwrap();
                    let token = files[0].token;
                    scope
                        .request(
                            Query::FileRead(FileRead {
                                file: token,
                                offset: 0,
                                length: 4,
                            }),
                            move |result| {
                                let Ok(QueryValue::FileBytes(chunk)) = result else {
                                    panic!("bounded read failed {result:?}")
                                };
                                header_state.revise(|v| *v = format!("{:?}", chunk.bytes));
                            },
                        )
                        .unwrap();
                    scope
                        .request(Query::FilePreview(token), move |result| {
                            let Ok(QueryValue::FilePreview(preview)) = result else {
                                panic!("preview failed {result:?}")
                            };
                            file_state.revise(|v| {
                                *v = preview.map_or_else(|| "rejected".into(), |p| p.source)
                            });
                        })
                        .unwrap();
                }),
            text().class("structured-preview").content(file_state),
            text().class("structured-header").content(header_state),
            input()
                .class("structured-input")
                .semantics(Some(Box::new(Semantics {
                    input_edit: Some(InputEditCapture { max_bytes: 8 }),
                    ..Default::default()
                })))
                .on(EventKind::InputEdit, move |event| {
                    let Event::InputEdit(event) = event else {
                        return;
                    };
                    let message = match event {
                        InputEdit::Value {
                            value,
                            before,
                            selection,
                            intent,
                            ..
                        } => format!(
                            "{intent:?} {value} {:?} {}",
                            before.map(|s| s.start),
                            selection.end
                        ),
                        InputEdit::Rejected { reason, .. } => format!("{reason:?}"),
                    };
                    edit_state.revise(|v| *v = message);
                }),
            text().class("structured-edit-state").content(edit_state),
            rich_editor()
                .class("structured-rich")
                .node_ref(node)
                .on(EventKind::RichEdit, move |event| {
                    let Event::RichEdit(snapshot) = event else {
                        return;
                    };
                    rich_state.revise(|v| *v = snapshot.document.plain_text());
                    rich_scope
                        .upgrade()
                        .unwrap()
                        .set_rich_document(node.get().unwrap(), snapshot.document)
                        .unwrap();
                })
                .on(EventKind::RichRejected, move |_| {
                    rich_state.revise(|v| *v = "rejected".into())
                }),
            text().class("structured-rich-state").content(rich_state),
        )),
    )
}
