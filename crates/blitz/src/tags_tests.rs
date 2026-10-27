use super::focus_tests::{focus, selection, settle};
use super::*;
use glory_core::Cage;
use glory_render::{
    Change, Patch, SelectionDirection, Semantics, TagHostRejection, TagSeparators, TagsCommand,
    TextSelection,
};
use glory_ui::{input, view};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
struct Clipboard {
    text: Mutex<Option<String>>,
    reads: AtomicUsize,
}
impl blitz_traits::shell::ShellProvider for Clipboard {
    fn get_clipboard_text(&self) -> Result<String, blitz_traits::shell::ClipboardError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.text
            .lock()
            .unwrap()
            .clone()
            .ok_or(blitz_traits::shell::ClipboardError)
    }
}
struct Fixture {
    host: WindowApp,
    id: NodeId,
    draft: Cage<String>,
    readonly: Cage<bool>,
    policy: Cage<Option<TagSeparators>>,
    events: Rc<RefCell<Vec<(TagsCommand, u64)>>>,
    clipboard: Arc<Clipboard>,
}
fn fixture(value: &str) -> Fixture {
    let mut host = WindowApp::new();
    host.renderer.set_viewport(400, 240, 1.).unwrap();
    host.renderer
        .set_css("input{width:250px;font:18px sans-serif}");
    let scope = host.app.scope();
    let draft = scope.cage(value.to_owned());
    let readonly = scope.cage(false);
    let policy = scope.cage(Some(TagSeparators::EnterCommaSemicolon));
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let reference = scope.cage(None);
    host.mount(
        view().fill(
            input()
                .name("Free tags")
                .value(draft)
                .node_ref(reference)
                .semantics(scope.bond(move || {
                    Some(Box::new(Semantics {
                        tags_input: policy.get(),
                        readonly: readonly.get(),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::ValueChanged, move |event| {
                    if let Event::ValueChanged { value, .. } = event {
                        draft.revise(|v| *v = value);
                    }
                })
                .on(EventKind::Tags, move |event| {
                    if let Event::Tags {
                        command, revision, ..
                    } = event
                    {
                        output.borrow_mut().push((command, revision));
                    }
                }),
        ),
    )
    .unwrap();
    let clipboard = Arc::new(Clipboard::default());
    host.renderer
        .0
        .borrow_mut()
        .dom
        .doc
        .set_shell_provider(clipboard.clone());
    let id = reference.get().unwrap();
    focus(&mut host, id);
    Fixture {
        host,
        id,
        draft,
        readonly,
        policy,
        events,
        clipboard,
    }
}
fn raw(host: &mut WindowApp, key: Key, modifiers: Modifiers, repeat: bool) {
    let text = match &key {
        Key::Character(s) => Some(s.as_str().into()),
        _ => None,
    };
    host.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
        key,
        code: keyboard_types::Code::Unidentified,
        modifiers,
        location: keyboard_types::Location::Standard,
        is_auto_repeating: repeat,
        is_composing: false,
        state: blitz_traits::events::KeyState::Pressed,
        text,
    }));
}
fn key(f: &mut Fixture, key: Key) {
    raw(&mut f.host, key, Modifiers::empty(), false);
    settle(&mut f.host);
}
fn backspace(f: &mut Fixture, modifiers: Modifiers) {
    #[cfg(target_os = "macos")]
    f.host
        .handle_ui_event(UiEvent::AppleStandardKeybinding("deleteBackward:".into()));
    raw(
        &mut f.host,
        Key::Named(NamedKey::Backspace),
        modifiers,
        false,
    );
    settle(&mut f.host);
}
fn paste(f: &mut Fixture, text: &str) {
    *f.clipboard.text.lock().unwrap() = Some(text.to_owned());
    let mods = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    raw(&mut f.host, Key::Character("v".into()), mods, false);
    settle(&mut f.host);
}
fn select(f: &mut Fixture, start: usize, end: usize) {
    f.host
        .app
        .scope()
        .set_input_selection(
            f.id,
            TextSelection {
                start,
                end,
                direction: SelectionDirection::Backward,
            },
        )
        .unwrap();
    f.host.app.flush().unwrap();
    settle(&mut f.host);
}
fn ime(f: &mut Fixture, event: BlitzImeEvent) {
    f.host.handle_ui_event(UiEvent::Ime(event));
    settle(&mut f.host);
}
fn commands(f: &Fixture) -> Vec<TagsCommand> {
    f.events.borrow().iter().map(|(c, _)| c.clone()).collect()
}

#[test]
fn native_tags_separators_patch_without_replacing_editor_or_consuming_ordinary_keys() {
    let mut f = fixture("中文 draft");
    let physical = f.host.renderer.engine_id(f.id, Part::Main).unwrap();
    select(&mut f, 2, 2);
    key(&mut f, Key::Character(",".into()));
    key(&mut f, Key::Character(";".into()));
    key(&mut f, Key::Named(NamedKey::Enter));
    assert_eq!(
        commands(&f),
        vec![
            TagsCommand::Commit {
                value: "中文 draft".into()
            };
            3
        ]
    );
    assert_eq!(selection(&mut f.host, f.id), (2, 2));
    assert_eq!(f.draft.get(), "中文 draft");
    f.policy.revise(|v| *v = Some(TagSeparators::Enter));
    f.host.app.flush().unwrap();
    key(&mut f, Key::Character(",".into()));
    assert_eq!(f.draft.get(), "中文, draft");
    assert_eq!(f.events.borrow().len(), 3);
    f.policy.revise(|v| *v = None);
    f.host.app.flush().unwrap();
    key(&mut f, Key::Character(";".into()));
    assert_eq!(f.draft.get(), "中文,; draft");
    assert_eq!(
        f.host.renderer.engine_id(f.id, Part::Main).unwrap(),
        physical
    );
}

#[test]
fn native_tags_empty_backspace_does_not_remove_a_tag_after_deleting_the_last_character() {
    let mut f = fixture("A");
    select(&mut f, 1, 1);
    backspace(&mut f, Modifiers::empty());
    assert_eq!(f.draft.get(), "");
    assert!(commands(&f).is_empty());
    backspace(&mut f, Modifiers::SHIFT);
    assert!(commands(&f).is_empty());
    backspace(&mut f, Modifiers::empty());
    assert_eq!(commands(&f), [TagsCommand::RemoveLast]);
    raw(
        &mut f.host,
        Key::Character(",".into()),
        Modifiers::empty(),
        true,
    );
    settle(&mut f.host);
    assert_eq!(
        f.events.borrow().len(),
        1,
        "repeat punctuation remains native"
    );
    f.readonly.revise(|v| *v = true);
    f.host.app.flush().unwrap();
    let value = f.draft.get();
    key(&mut f, Key::Character(";".into()));
    assert_eq!(f.draft.get(), value);
    assert_eq!(f.events.borrow().len(), 1);
}

#[test]
fn native_tags_delimited_paste_is_one_utf16_proposal_without_editor_mutation() {
    let mut f = fixture("A😀B");
    select(&mut f, 1, 3);
    paste(&mut f, "中,第二\r\nthird");
    assert_eq!(
        commands(&f),
        [TagsCommand::Paste {
            value: "A😀B".into(),
            text: "中,第二\nthird".into(),
            start: 1,
            end: 3
        }]
    );
    assert_eq!(f.draft.get(), "A😀B");
    assert_eq!(selection(&mut f.host, f.id), (1, 3));
    assert_eq!(f.clipboard.reads.load(Ordering::Relaxed), 1);
    let raw_selection = *f
        .host
        .renderer
        .0
        .borrow()
        .dom
        .doc
        .get_node(f.host.renderer.engine_id(f.id, Part::Main).unwrap())
        .unwrap()
        .element_data()
        .unwrap()
        .text_input_data()
        .unwrap()
        .editor
        .raw_selection();
    assert!(raw_selection.anchor().index() > raw_selection.focus().index());
}

#[test]
fn native_tags_paste_budgets_are_body_free_preserve_selection_and_recover_with_one_read() {
    let mut f = fixture("A😀B");
    select(&mut f, 1, 3);
    paste(&mut f, &"x".repeat(1_000_000));
    assert_eq!(
        commands(&f),
        [TagsCommand::Reject(TagHostRejection::ClipboardBudget)]
    );
    assert_eq!(selection(&mut f.host, f.id), (1, 3));
    assert_eq!(f.draft.get(), "A😀B");
    paste(&mut f, &"中".repeat(2000));
    assert_eq!(
        commands(&f).last(),
        Some(&TagsCommand::Reject(TagHostRejection::InputBudget))
    );
    assert_eq!(f.draft.get(), "A😀B");
    paste(&mut f, "plain 中");
    assert_eq!(f.draft.get(), "Aplain 中B");
    assert_eq!(f.clipboard.reads.load(Ordering::Relaxed), 3);
    assert_eq!(
        f.events.borrow().len(),
        2,
        "ordinary paste is text editing, without a Tags command"
    );
    *f.clipboard.text.lock().unwrap() = None;
    let before = f.draft.get();
    let mods = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    raw(&mut f.host, Key::Character("v".into()), mods, false);
    settle(&mut f.host);
    assert_eq!(f.draft.get(), before);
    f.host.app.clear().unwrap();
    assert_eq!(f.host.renderer.stats().nodes, 0);
}

#[test]
fn native_tags_mobile_punctuation_and_return_preserve_composition_and_terminal_candidates() {
    let mut f = fixture("中文");
    ime(&mut f, BlitzImeEvent::Commit(",".into()));
    assert_eq!(
        commands(&f),
        [TagsCommand::Commit {
            value: "中文".into()
        }]
    );
    assert_eq!(f.draft.get(), "中文");
    ime(&mut f, BlitzImeEvent::Commit("\n".into()));
    assert_eq!(f.events.borrow().len(), 2);
    select(&mut f, 0, 2);
    ime(&mut f, BlitzImeEvent::Preedit(",".into(), Some((1, 1))));
    key(&mut f, Key::Character(";".into()));
    assert_eq!(f.events.borrow().len(), 2);
    ime(&mut f, BlitzImeEvent::Preedit("".into(), None));
    ime(&mut f, BlitzImeEvent::Commit(",".into()));
    assert_eq!(
        f.events.borrow().len(),
        2,
        "a terminal IME candidate is never a tag separator"
    );
    assert_eq!(f.draft.get(), ",");
    key(&mut f, Key::Named(NamedKey::Enter));
    assert_eq!(
        commands(&f).last(),
        Some(&TagsCommand::Commit { value: ",".into() })
    );
}

#[cfg(target_os = "macos")]
#[test]
fn native_tags_appkit_paste_is_not_repeated_by_its_physical_key_and_markers_retire() {
    let mut f = fixture("draft");
    *f.clipboard.text.lock().unwrap() = Some("a,b".into());
    f.host
        .handle_ui_event(UiEvent::AppleStandardKeybinding("paste:".into()));
    raw(
        &mut f.host,
        Key::Character("v".into()),
        Modifiers::META,
        false,
    );
    settle(&mut f.host);
    assert_eq!(f.clipboard.reads.load(Ordering::Relaxed), 1);
    assert_eq!(f.events.borrow().len(), 1);
    assert!(f.host.renderer.0.borrow().apple_input.is_none());
    f.host
        .handle_ui_event(UiEvent::AppleStandardKeybinding("paste:".into()));
    settle(&mut f.host);
    f.policy.revise(|v| *v = None);
    f.host.app.flush().unwrap();
    raw(
        &mut f.host,
        Key::Character("v".into()),
        Modifiers::META,
        false,
    );
    settle(&mut f.host);
    assert_eq!(
        f.clipboard.reads.load(Ordering::Relaxed),
        2,
        "a consumed AppKit paste must not repeat after its policy retires"
    );
    assert_eq!(f.draft.get(), "draft");
    f.policy
        .revise(|v| *v = Some(TagSeparators::EnterCommaSemicolon));
    f.host.app.flush().unwrap();
    f.host
        .handle_ui_event(UiEvent::AppleStandardKeybinding("insertNewline:".into()));
    raw(
        &mut f.host,
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        false,
    );
    settle(&mut f.host);
    assert_eq!(f.events.borrow().len(), 3);
    f.draft.revise(|v| v.clear());
    f.host.app.flush().unwrap();
    f.host
        .handle_ui_event(UiEvent::AppleStandardKeybinding("deleteBackward:".into()));
    assert!(f.host.renderer.0.borrow().apple_input.is_some());
    f.host.app.clear().unwrap();
    assert!(f.host.renderer.0.borrow().apple_input.is_none());
}

#[test]
fn native_tags_policy_refuses_inert_and_hidden_origins_without_clipboard_reads() {
    let mut f = fixture("draft");
    let root = f.host.app.tree().roots()[0];
    for metadata in [
        Semantics {
            inert: true,
            ..Default::default()
        },
        Semantics {
            hidden: true,
            ..Default::default()
        },
    ] {
        f.host
            .app
            .scope()
            .patch(
                root,
                Patch::Semantics(Change::Set(Some(Box::new(metadata)))),
            )
            .unwrap();
        f.host.app.flush().unwrap();
        let target = f.host.renderer.engine_id(f.id, Part::Main).unwrap();
        let renderer = f.host.renderer.clone();
        EventDriver::new(&mut f.host, Events::new(renderer)).handle_dom_event(DomEvent::new(
            target,
            DomEventData::KeyDown(BlitzKeyEvent {
                key: Key::Character(",".into()),
                code: keyboard_types::Code::Unidentified,
                modifiers: Modifiers::empty(),
                location: keyboard_types::Location::Standard,
                is_auto_repeating: false,
                is_composing: false,
                state: blitz_traits::events::KeyState::Pressed,
                text: Some(",".into()),
            }),
        ));
        settle(&mut f.host);
        assert!(commands(&f).is_empty());
        assert_eq!(f.clipboard.reads.load(Ordering::Relaxed), 0);
        f.host
            .app
            .scope()
            .patch(root, Patch::Semantics(Change::Clear))
            .unwrap();
        f.host.app.flush().unwrap();
        focus(&mut f.host, f.id);
    }
}
