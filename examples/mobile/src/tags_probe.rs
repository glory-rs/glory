//! Protocol display/input fixture; actual widget policy is tested with Sapid.
use glory::prelude::*;
use glory::render::{EventKind, Semantics, TagSeparators, TagsCommand};

pub fn fixture() -> impl Widget {
    widget(|scope| {
        let message = scope.cage("No tag command yet".to_owned());
        let mut editors: Vec<Box<dyn Widget>> = Vec::new();
        for (label, initial, separators) in [
            ("Enter only", "Rust", TagSeparators::Enter),
            ("Enter or comma", "中文", TagSeparators::EnterComma),
            (
                "Enter, comma or semicolon",
                "Glory",
                TagSeparators::EnterCommaSemicolon,
            ),
        ] {
            let draft = scope.cage(initial.to_owned());
            editors.push(Box::new(
                view().fill((
                    text().content(label),
                    input()
                        .name(label)
                        .value(draft)
                        .semantics(Some(Box::new(Semantics {
                            tags_input: Some(separators),
                            placeholder: Some("Free draft / 中文".into()),
                            ..Default::default()
                        })))
                        .on(EventKind::ValueChanged, move |event| {
                            if let Event::ValueChanged { value, .. } = event {
                                draft.revise(|v| *v = value);
                            }
                        })
                        .on(EventKind::Tags, move |event| {
                            if let Event::Tags { command, .. } = event {
                                let status = match command {
                                    TagsCommand::Commit { value } => {
                                        draft.revise(String::clear);
                                        format!("Commit {} UTF-8 bytes", value.len())
                                    }
                                    TagsCommand::RemoveLast => "RemoveLast".to_owned(),
                                    TagsCommand::Paste {
                                        text, start, end, ..
                                    } => format!(
                                        "Paste {} bytes at UTF-16 {start}..{end}",
                                        text.len()
                                    ),
                                    TagsCommand::Reject(reason) => format!("Rejected {reason:?}"),
                                };
                                message.revise(|v| *v = status);
                            }
                        }),
                )),
            ));
        }
        view()
            .class("tags-probe")
            .fill((
                text().content("Native free-tag input protocol"),
                group().fill(editors),
                text().content(message),
                text().content(
                    "Composition stays in the editor. Paste requires a clipboard provider.",
                ),
            ))
            .show_in(scope)
    })
}

pub const CSS: &str = ".tags-probe{font:18px sans-serif;display:flex;flex-direction:column;gap:8px;padding:24px;box-sizing:border-box}.tags-probe>div{display:flex;flex-direction:column;gap:8px}.tags-probe input{display:block;box-sizing:border-box;max-width:100%;width:270px;height:42px;padding:6px}";
