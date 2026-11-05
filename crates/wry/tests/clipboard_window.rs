//! Opt-in native plain-text clipboard completion and editor preservation.
use glory_render::{ClipboardOutcome, ClipboardText, Semantics};
use glory_ui::{events::*, *};
use glory_wry::shell::*;
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    if std::env::var("GLORY_WRY_CLIPBOARD").as_deref() != Ok("1") {
        println!("Set GLORY_WRY_CLIPBOARD=1 for native clipboard acceptance");
        return Ok(());
    }
    let app = WindowApp::new();
    app.renderer().enable_text_clipboard();
    let node = app.app().scope().cage(None);
    let weak = app.app().scope().downgrade();
    let done = Rc::new(Cell::new(0));
    let received = Rc::new(Cell::new(false));
    let receipt = received.clone();
    let status = done.clone();
    let done_copy = done.clone();
    app.mount(
        col().fill((
            input().value("中文😀 original caret").class("editor"),
            button()
                .label("Copy literal")
                .node_ref(node)
                .semantics(Some(Box::new(Semantics {
                    clipboard: Some(ClipboardText {
                        text: "literal <script>😀".into(),
                        revision: 1,
                    }),
                    ..Default::default()
                })))
                .on(activate, move |_| {
                    let scope = weak.upgrade().unwrap();
                    scope.copy_text(node.get().unwrap()).unwrap();
                })
                .on(glory_render::EventKind::Clipboard, move |event| {
                    if let Event::Clipboard { outcome, .. } = event {
                        if outcome == ClipboardOutcome::Written {
                            receipt.set(true);
                        } else {
                            done_copy.set(-1);
                        }
                    }
                }),
            input().class("status").on(value_changed, move |event| {
                if let Event::ValueChanged { value, .. } = event {
                    if value == "pass" {
                        status.set(2);
                    } else if value.starts_with("FAIL") {
                        status.set(-1);
                    }
                }
            }),
        )),
    )?;
    let start = Instant::now();
    Shell::new().run_until(app,Config {title:"Glory owned clipboard acceptance".into(),initialization_script:Some(r#"document.addEventListener('DOMContentLoaded',()=>{setTimeout(()=>{const input=document.querySelector('.editor input')||document.querySelector('input.editor');input.focus();input.setSelectionRange(2,4);document.querySelector('button').click();setTimeout(()=>{const result=document.querySelector('.status input')||document.querySelector('input.status');result.value=document.activeElement===input&&input.selectionStart===2&&input.selectionEnd===4?'pass':'FAIL selection';result.dispatchEvent(new Event('input',{bubbles:true}));},700);},500);});"#.into()),..Default::default()},move |_|{
        if done.get()<0||start.elapsed()>Duration::from_secs(8){return Err(ShellError("Native clipboard completion/selection failed".into()));}
        if done.get()==2 && received.get() {println!("Native plain-text clipboard outcome and original UTF-16 editor selection passed");return Ok(true);}Ok(false)
    })?;
    Ok(())
}
