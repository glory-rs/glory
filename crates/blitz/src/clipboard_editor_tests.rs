use super::focus_tests::{focus, key, selection, settle};
use super::*;
use crate::clipboard::{Clipboard, ClipboardDriver, ClipboardError, Reply};
use glory_core::Cage;
use glory_render::{
    Change, InputEditCapture, Patch, Purpose, SelectionDirection, Semantics, TagSeparators,
    TagsCommand, TextSelection,
};
use glory_ui::{input, view};
use std::{
    future::Future,
    pin::Pin,
    sync::Mutex,
    task::{Poll, Waker},
};

#[derive(Default)]
struct Gate {
    value: Option<Result<String, ClipboardError>>,
    waker: Option<Waker>,
}
struct Wait(Arc<Mutex<Gate>>);
impl Future for Wait {
    type Output = Result<String, ClipboardError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut gate = self.0.lock().unwrap();
        if let Some(result) = gate.value.take() {
            return Poll::Ready(result);
        }
        let previous = gate.waker.replace(cx.waker().clone());
        drop(gate);
        drop(previous);
        Poll::Pending
    }
}
impl Drop for Wait {
    fn drop(&mut self) {
        let wake = self.0.lock().unwrap().waker.take();
        drop(wake);
    }
}
enum Call {
    Read(usize),
    Write(String),
}
#[derive(Default)]
struct Driver {
    calls: Mutex<Vec<(Call, Arc<Mutex<Gate>>)>>,
}
impl Driver {
    fn begin(&self, call: Call) -> Wait {
        let gate = Arc::new(Mutex::new(Gate::default()));
        self.calls.lock().unwrap().push((call, gate.clone()));
        Wait(gate)
    }
    fn len(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    fn finish(&self, n: usize, result: Result<String, ClipboardError>) {
        let gate = self.calls.lock().unwrap()[n].1.clone();
        let wake = {
            let mut gate = gate.lock().unwrap();
            gate.value = Some(result);
            gate.waker.take()
        };
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}
impl ClipboardDriver for Driver {
    fn write(&self, text: String) -> Reply<()> {
        let wait = self.begin(Call::Write(text));
        Box::pin(async move { wait.await.map(|_| ()) })
    }
    fn read(&self, limit: usize) -> Reply<String> {
        Box::pin(self.begin(Call::Read(limit)))
    }
}
struct Fixture {
    host: WindowApp,
    id: NodeId,
    driver: Arc<Driver>,
    draft: Cage<String>,
    readonly: Cage<bool>,
    purpose: Cage<Purpose>,
    policy: Cage<Option<TagSeparators>>,
    events: Rc<RefCell<Vec<Event>>>,
}
fn fixture(value: &str, tags: bool, capture: Option<InputEditCapture>) -> Fixture {
    let driver = Arc::new(Driver::default());
    let mut host = WindowApp::with_config(Config {
        clipboard: Some(Clipboard::new(driver.clone())),
        ..Default::default()
    });
    let scope = host.app.scope();
    let draft = scope.cage(value.to_owned());
    let readonly = scope.cage(false);
    let purpose = scope.cage(Purpose::Text);
    let policy = scope.cage(tags.then_some(TagSeparators::EnterCommaSemicolon));
    let reference = scope.cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let changed = events.clone();
    let tagged = events.clone();
    let edited = events.clone();
    let mut editor = input()
        .name("Clipboard editor")
        .node_ref(reference)
        .value(draft)
        .purpose(purpose)
        .semantics(scope.bond(move || {
            Some(Box::new(Semantics {
                readonly: readonly.get(),
                tags_input: policy.get(),
                input_edit: capture,
                ..Default::default()
            }))
        }))
        .on(EventKind::ValueChanged, move |event| {
            if let Event::ValueChanged { value, .. } = &event {
                draft.revise(|v| *v = value.clone());
            }
            changed.borrow_mut().push(event);
        });
    if tags {
        editor = editor.on(EventKind::Tags, move |event| {
            tagged.borrow_mut().push(event);
        });
    }
    if capture.is_some() {
        editor = editor.on(EventKind::InputEdit, move |event| {
            if let Event::InputEdit(glory_render::InputEdit::Value { value, .. }) = &event {
                draft.revise(|v| *v = value.clone());
            }
            edited.borrow_mut().push(event);
        });
    }
    host.mount(view().fill(editor)).unwrap();
    let id = reference.get().unwrap();
    focus(&mut host, id);
    Fixture {
        host,
        id,
        driver,
        draft,
        readonly,
        purpose,
        policy,
        events,
    }
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
fn raw(f: &mut Fixture, letter: &str) {
    f.host.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
        key: Key::Character(letter.into()),
        code: keyboard_types::Code::Unidentified,
        modifiers: if cfg!(any(target_os = "macos", target_os = "ios")) {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        },
        location: keyboard_types::Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: blitz_traits::events::KeyState::Pressed,
        text: None,
    }));
}
fn command(f: &mut Fixture, letter: &str) {
    raw(f, letter);
    settle(&mut f.host);
}
fn finish(f: &mut Fixture, n: usize, result: Result<String, ClipboardError>) {
    f.driver.finish(n, result);
    settle(&mut f.host);
}

#[test]
fn configured_clipboard_pastes_once_and_cuts_only_after_successful_write() {
    let mut f = fixture("A中😀Z", false, None);
    select(&mut f, 1, 4);
    raw(&mut f, "v");
    assert_eq!(f.driver.len(), 0);
    settle(&mut f.host);
    assert!(matches!(
        f.driver.calls.lock().unwrap()[0].0,
        Call::Read(glory_render::MAX_CLIPBOARD_BYTES)
    ));
    finish(&mut f, 0, Ok("文\r\n字\0".into()));
    assert_eq!(f.draft.get(), "A文字�Z");
    assert_eq!(selection(&mut f.host, f.id), (4, 4));
    select(&mut f, 1, 3);
    command(&mut f, "x");
    assert_eq!(f.draft.get(), "A文字�Z");
    assert!(matches!(&f.driver.calls.lock().unwrap()[1].0, Call::Write(text) if text == "文字"));
    finish(&mut f, 1, Err(ClipboardError::Denied));
    assert_eq!(f.draft.get(), "A文字�Z");
    command(&mut f, "x");
    finish(&mut f, 2, Ok(String::new()));
    assert_eq!(f.draft.get(), "A�Z");
    assert_eq!(f.events.borrow().len(), 2);
}

#[test]
fn pending_clipboard_does_not_overwrite_a_changed_editor_or_refund_its_permit() {
    let mut f = fixture("first", false, None);
    select(&mut f, 0, 5);
    command(&mut f, "v");
    key(&mut f.host, Key::Character("x".into()), Modifiers::empty());
    command(&mut f, "v");
    assert_eq!(f.driver.len(), 1);
    finish(&mut f, 0, Ok("stale".into()));
    assert_eq!(f.draft.get(), "x");
    command(&mut f, "v");
    finish(&mut f, 1, Ok("new".into()));
    assert_eq!(f.draft.get(), "xnew");
}

#[test]
fn pending_cut_and_paste_ignore_selection_focus_and_program_value_aba() {
    for change in 0..3 {
        let mut f = fixture("A中😀Z", false, None);
        select(&mut f, 1, 4);
        command(&mut f, "x");
        match change {
            0 => {
                select(&mut f, 0, 0);
                select(&mut f, 1, 4);
            }
            1 => {
                f.host.renderer.0.borrow_mut().dom.doc.clear_focus();
                f.host.renderer.0.borrow_mut().sync_focus();
                settle(&mut f.host);
                focus(&mut f.host, f.id);
                select(&mut f, 1, 4);
            }
            _ => {
                f.draft.revise(|v| *v = "other".into());
                f.host.app.flush().unwrap();
                f.draft.revise(|v| *v = "A中😀Z".into());
                f.host.app.flush().unwrap();
                select(&mut f, 1, 4);
            }
        }
        finish(&mut f, 0, Ok(String::new()));
        assert_eq!(f.draft.get(), "A中😀Z");
    }
}

#[test]
fn clipboard_respects_readonly_password_ime_and_budget_without_provider_fallback() {
    let mut f = fixture("secret😀", false, None);
    select(&mut f, 0, 8);
    f.purpose.revise(|p| *p = Purpose::Password);
    f.host.app.flush().unwrap();
    command(&mut f, "c");
    command(&mut f, "x");
    assert_eq!(f.driver.len(), 0);
    command(&mut f, "v");
    finish(&mut f, 0, Ok("public".into()));
    assert_eq!(f.draft.get(), "public");
    f.purpose.revise(|p| *p = Purpose::Text);
    f.readonly.revise(|v| *v = true);
    f.host.app.flush().unwrap();
    select(&mut f, 0, 6);
    command(&mut f, "v");
    command(&mut f, "x");
    assert_eq!(f.driver.len(), 1);
    command(&mut f, "c");
    finish(&mut f, 1, Ok(String::new()));
    assert!(matches!(&f.driver.calls.lock().unwrap()[1].0, Call::Write(text) if text == "public"));
    f.readonly.revise(|v| *v = false);
    f.host.app.flush().unwrap();
    command(&mut f, "v");
    finish(
        &mut f,
        2,
        Ok("a".repeat(glory_render::MAX_CLIPBOARD_BYTES + 1)),
    );
    assert_eq!(f.draft.get(), "public");
    command(&mut f, "v");
    f.host.handle_ui_event(UiEvent::Ime(BlitzImeEvent::Preedit(
        "拼".into(),
        Some((3, 3)),
    )));
    settle(&mut f.host);
    finish(&mut f, 3, Ok("late".into()));
    assert!(!f.draft.get().contains("late"));
}

#[test]
fn async_tag_paste_reuses_atomic_shared_protocol_and_body_free_budget_rejection() {
    let mut f = fixture("A中😀Z", true, None);
    select(&mut f, 1, 4);
    command(&mut f, "v");
    assert!(matches!(
        f.driver.calls.lock().unwrap()[0].0,
        Call::Read(glory_render::MAX_TAG_PASTE_BYTES)
    ));
    finish(&mut f, 0, Ok("one,two".into()));
    assert_eq!(f.draft.get(), "A中😀Z");
    assert!(
        matches!(&f.events.borrow()[0], Event::Tags { command: TagsCommand::Paste { value, text, start: 1, end: 4 }, .. } if value == "A中😀Z" && text == "one,two")
    );
    command(&mut f, "v");
    finish(&mut f, 1, Err(ClipboardError::TooLong));
    assert!(matches!(
        &f.events.borrow()[1],
        Event::Tags {
            command: TagsCommand::Reject(glory_render::TagHostRejection::ClipboardBudget),
            ..
        }
    ));
    assert_eq!(selection(&mut f.host, f.id), (1, 4));
    command(&mut f, "v");
    finish(&mut f, 2, Ok("文".into()));
    assert_eq!(f.draft.get(), "A文Z");
}

#[test]
fn appkit_command_and_raw_key_share_one_receipt_even_after_tag_policy_retirement() {
    let mut f = fixture("draft", true, None);
    f.host
        .handle_ui_event(UiEvent::AppleStandardKeybinding("paste:".into()));
    settle(&mut f.host);
    finish(&mut f, 0, Ok("one,two".into()));
    f.policy.revise(|p| *p = None);
    f.host.app.flush().unwrap();
    command(&mut f, "v");
    assert_eq!(f.driver.len(), 1);
    command(&mut f, "v");
    assert_eq!(f.driver.len(), 2);
}

#[test]
fn clipboard_deadlines_and_retirement_prevent_both_lazy_start_and_late_mutation() {
    let mut f = fixture("old", false, None);
    command(&mut f, "v");
    f.host.renderer.0.borrow_mut().clipboard.expire();
    f.driver.finish(0, Ok("late".into()));
    settle(&mut f.host);
    assert_eq!(f.draft.get(), "old");
    raw(&mut f, "v");
    f.host.renderer.0.borrow_mut().clipboard.expire();
    settle(&mut f.host);
    assert_eq!(f.driver.len(), 1);
    command(&mut f, "v");
    assert_eq!(f.driver.len(), 2);
    f.host.app.clear().unwrap();
    finish(&mut f, 1, Ok("retired".into()));
    assert_eq!(f.host.renderer.stats().nodes, 0);
}

#[test]
fn bounded_input_edit_paste_preserves_before_selection_and_refuses_before_mutation() {
    let mut f = fixture("A😀Z", false, Some(InputEditCapture { max_bytes: 16 }));
    select(&mut f, 1, 3);
    command(&mut f, "v");
    finish(&mut f, 0, Ok("中".into()));
    assert_eq!(f.draft.get(), "A中Z");
    assert!(matches!(
        &f.events.borrow()[0],
        Event::InputEdit(glory_render::InputEdit::Value {
            before: Some(TextSelection {
                start: 1,
                end: 3,
                direction: SelectionDirection::Backward
            }),
            selection: TextSelection {
                start: 2,
                end: 2,
                ..
            },
            ..
        })
    ));
    command(&mut f, "v");
    finish(&mut f, 1, Ok("x".repeat(16)));
    assert_eq!(f.draft.get(), "A中Z");
    assert_eq!(selection(&mut f.host, f.id), (2, 2));
    assert!(matches!(
        &f.events.borrow()[1],
        Event::InputEdit(glory_render::InputEdit::Rejected {
            reason: glory_render::InputEditRejection::TooLong,
            ..
        })
    ));
}

#[test]
fn closing_document_drops_the_driver_waiter_without_retaining_its_waker() {
    let mut f = fixture("draft", false, None);
    command(&mut f, "v");
    let driver = f.driver.clone();
    let gate = driver.calls.lock().unwrap()[0].1.clone();
    assert!(gate.lock().unwrap().waker.is_some());
    drop(f);
    assert!(gate.lock().unwrap().waker.is_none());
    driver.finish(0, Ok("late".into()));
}

#[test]
fn pending_paste_survives_presentation_updates_without_using_layout_generation_as_edit_identity() {
    let mut f = fixture("A中😀Z", false, None);
    select(&mut f, 1, 4);
    command(&mut f, "v");
    let physical = f.host.renderer.engine_id(f.id, Part::Main).unwrap();
    f.host
        .app
        .scope()
        .patch(
            f.id,
            Patch::Name(Change::Set(Some("New accessible name".into()))),
        )
        .unwrap();
    f.host
        .app
        .scope()
        .patch(
            f.id,
            Patch::Style(Change::Set(Box::new(
                glory_render::style::Style::new().width(glory_render::style::Length::Px(210.)),
            ))),
        )
        .unwrap();
    f.host.app.flush().unwrap();
    settle(&mut f.host);
    finish(&mut f, 0, Ok("accepted".into()));
    assert_eq!(f.draft.get(), "AacceptedZ");
    assert_eq!(
        f.host.renderer.engine_id(f.id, Part::Main).unwrap(),
        physical
    );
}
