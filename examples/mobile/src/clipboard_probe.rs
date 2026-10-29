//! Owned emulator acceptance; no user clipboard body is logged or persisted.
use glory::apace::clipboard::{AndroidClipboard, ClipboardDriver, ClipboardError};
use glory::prelude::*;
use glory::render::{ClipboardOutcome, ClipboardText, Semantics};
use jni::{JavaVM, jni_sig, jni_str, objects::JByteArray};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn prepare() -> std::result::Result<Option<String>, Box<dyn std::error::Error>> {
    JavaVM::singleton()?.attach_current_thread(
        |env| -> std::result::Result<_, Box<dyn std::error::Error>> {
            let class = env.load_class(jni_str!("rs.glory.counter.GloryGameActivity"))?;
            let object = env
                .call_static_method(
                    class,
                    jni_str!("gloryClipboardProbePrepare"),
                    jni_sig!(() -> [i8]),
                    &[],
                )?
                .l()?;
            let bytes = JByteArray::cast_local(env, object)?;
            if bytes.is_null() {
                return Ok(None);
            }
            if bytes.len(env)? > 96 {
                return Err("invalid owned clipboard token".into());
            }
            Ok(Some(String::from_utf8(env.convert_byte_array(&bytes)?)?))
        },
    )
}
fn restore() -> std::result::Result<i64, Box<dyn std::error::Error>> {
    JavaVM::singleton()?.attach_current_thread(
        |env| -> std::result::Result<_, Box<dyn std::error::Error>> {
            let class = env.load_class(jni_str!("rs.glory.counter.GloryGameActivity"))?;
            Ok(env
                .call_static_method(
                    class,
                    jni_str!("gloryClipboardProbeFinish"),
                    jni_sig!(() -> i64),
                    &[],
                )?
                .j()?)
        },
    )
}
async fn pause() {
    futures_timer::Delay::new(Duration::from_millis(8)).await;
}
pub(crate) fn fixture(driver: Arc<AndroidClipboard>) -> impl Widget {
    widget(move |scope| {
        let status = scope.cage("Waiting for foreground clipboard guard".to_owned());
        let reference = scope.cage(None);
        let completed = scope.cage(None::<ClipboardOutcome>);
        col()
            .style(Style::new().padding(Edges::all(24.)).gap(12.))
            .fill((
                text().content("Android Clipboard Probe"),
                text().content(status),
                button()
                    .label("Copy owned source")
                    .node_ref(reference)
                    .semantics(Some(Box::new(Semantics {
                        clipboard: Some(ClipboardText {
                            text: String::new(),
                            revision: 0,
                        }),
                        ..Default::default()
                    })))
                    .on(glory::render::EventKind::Clipboard, move |event| {
                        if let glory::render::Event::Clipboard { outcome, .. } = event {
                            completed.revise(|v| *v = Some(outcome));
                        }
                    }),
                input().name("Clipboard editor").value("A中😀Z"),
            ))
            .show_in(scope)?;
        let weak = scope.downgrade();
        let driver = driver.clone();
        scope.owner().spawn(async move {
            let result = async {
                let deadline = Instant::now() + Duration::from_secs(10);
                let token = loop {
                    if let Some(token) = prepare()? {
                        break token;
                    }
                    if Instant::now() > deadline {
                        return Err("foreground snapshot timed out".into());
                    }
                    pause().await;
                };
                let text = format!("{token}:中😀\r\nsecond\0");
                let future = driver.write(text.clone());
                if driver.stats()? != 0 {
                    return Err("lazy future started native work".into());
                }
                drop(future);
                driver.write(text.clone()).await?;
                let expected = glory::render::normalize_text(&text, true).into_owned();
                if driver.read(65536).await? != expected {
                    return Err("owned round-trip mismatch".into());
                }
                if !matches!(driver.read(8).await, Err(ClipboardError::TooLong)) {
                    return Err("read budget missing".into());
                }
                if !matches!(
                    driver.write("x".repeat(65537)).await,
                    Err(ClipboardError::TooLong)
                ) {
                    return Err("write budget missing".into());
                }
                let Some(scope) = weak.upgrade() else {
                    return Err("probe retired".into());
                };
                let id = reference.get().ok_or("copy node missing")?;
                scope.patch(
                    id,
                    glory::render::Patch::Semantics(glory::render::Change::Set(Some(Box::new(
                        Semantics {
                            clipboard: Some(ClipboardText {
                                text: expected.clone(),
                                revision: 1,
                            }),
                            ..Default::default()
                        },
                    )))),
                )?;
                scope.copy_text(id)?;
                drop(scope);
                let deadline = Instant::now() + Duration::from_secs(10);
                loop {
                    if let Some(outcome) = completed.get() {
                        if outcome != ClipboardOutcome::Written {
                            return Err("semantic copy failed".into());
                        }
                        break;
                    }
                    if Instant::now() > deadline {
                        return Err("semantic copy timed out".into());
                    }
                    pause().await;
                }
                if driver.read(65536).await? != expected {
                    return Err("semantic clipboard mismatch".into());
                }
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    match restore()? {
                        1 => break,
                        0 if Instant::now() < deadline => pause().await,
                        _ => return Err(
                            "original clipboard restoration failed or external change preserved"
                                .into(),
                        ),
                    }
                }
                if driver.stats()? != 0 {
                    return Err("native records did not drain".into());
                }
                Ok::<_, Box<dyn std::error::Error>>(())
            }
            .await;
            // All errors are body-free; never format a returned OS String.
            eprintln!("Glory Android clipboard acceptance: {result:?}");
            status.revise(|v| {
                *v = if result.is_ok() {
                    "Clipboard read/write, semantic copy, budgets and restoration passed".into()
                } else {
                    "Clipboard acceptance failed; inspect body-free diagnostics".into()
                }
            });
        })?;
        Ok(())
    })
}
