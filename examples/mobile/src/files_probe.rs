//! Owned activity-result/provider acceptance, separate from system dialog input.
use glory::blitz::{
    Renderer,
    files::{AndroidFiles, FilePicker, Files},
};
use glory::prelude::*;
use glory::render::{
    FileConfig, FileFilter, FileInfo, FileOutcome, FileRead, FileRejection, Query, QueryValue,
};
use glory::ui::file;
use jni::{JavaVM, jni_sig, jni_str, objects::JValue};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

type ProbeResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn native<T>(result: std::result::Result<T, FileRejection>) -> ProbeResult<T> {
    result.map_err(|error| std::io::Error::other(format!("native files: {error:?}")).into())
}
async fn pause() {
    futures_timer::Delay::new(Duration::from_millis(8)).await;
}
fn configure(mode: i32) -> ProbeResult {
    JavaVM::singleton()?.attach_current_thread(|env| -> ProbeResult {
        let class = env.load_class(jni_str!("rs.glory.counter.GloryFilesSelectionProbe"))?;
        env.call_static_method(
            class,
            jni_str!("configure"),
            jni_sig!((i32) -> ()),
            &[JValue::Int(mode)],
        )?;
        Ok(())
    })
}
fn provider_handles() -> ProbeResult<i32> {
    JavaVM::singleton()?.attach_current_thread(|env| -> ProbeResult<_> {
        let class = env.load_class(jni_str!("rs.glory.counter.GloryFilesProbeProvider"))?;
        Ok(env
            .call_static_method(class, jni_str!("handles"), jni_sig!(() -> i32), &[])?
            .i()?)
    })
}
async fn foreground() -> ProbeResult {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let ready = JavaVM::singleton()?.attach_current_thread(|env| -> ProbeResult<_> {
            let class = env.load_class(jni_str!("rs.glory.counter.GloryGameActivity"))?;
            Ok(env
                .call_static_method(
                    class,
                    jni_str!("gloryFileProbeReady"),
                    jni_sig!(() -> bool),
                    &[],
                )?
                .z()?)
        })?;
        if ready {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err("file host foreground timed out".into());
        }
        pause().await;
    }
}
async fn take<T: Clone + 'static>(value: Cage<Option<T>>) -> ProbeResult<T> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(result) = value.get() {
            return Ok(result);
        }
        if Instant::now() > deadline {
            return Err("file reply timed out".into());
        }
        pause().await;
    }
}
pub(crate) fn fixture(driver: Arc<AndroidFiles>, files: Files, renderer: Renderer) -> impl Widget {
    widget(move |scope| {
        let status = scope.cage("Waiting for Android file host".to_owned());
        let reference = scope.cage(None);
        let selected = scope.cage(None::<Vec<FileInfo>>);
        let preview = scope.cage(None::<String>);
        col()
            .style(Style::new().padding(Edges::all(24.)).gap(12.))
            .fill((
                text().content("Android Files Probe"),
                text().content(status),
                file()
                    .label("Choose owned documents")
                    .node_ref(reference)
                    .configuration(FileConfig {
                        multiple: true,
                        max_files: 2,
                        preview: Some(Default::default()),
                        ..Default::default()
                    })
                    .on(glory::render::EventKind::Files, move |event| {
                        if let Event::Files(event) = event {
                            if let FileOutcome::Selected(files) = event.outcome {
                                selected.revise(|v| *v = Some(files));
                            } else {
                                status.revise(|v| *v = "Semantic picker rejected".into());
                            }
                        }
                    }),
                when(preview.map(|value| value.is_some()), move || {
                    image()
                        .source(preview.get().expect("live preview branch"))
                        .decorative()
                        .style(Style::new().width(Length::Px(64.)).height(Length::Px(64.)))
                }),
            ))
            .show_in(scope)?;
        let weak = scope.downgrade();
        let driver = driver.clone();
        let files = files.clone();
        let renderer = renderer.clone();
        scope.owner().spawn(async move {
            let result = async {
                foreground().await?;
                let lazy = driver.pick(FileConfig::default());
                if native(driver.stats())?.picker != 0 {
                    return Err("lazy picker started work".into());
                }
                drop(lazy);
                let single = native(driver.pick(FileConfig::default()).await)?
                    .ok_or("owned selection missing")?;
                if single.len() != 1 || single[0].name != "badge.png" {
                    return Err("single metadata mismatch".into());
                }
                if native(single[0].reader.read(0, 8).await)? != b"\x89PNG\r\n\x1a\n" {
                    return Err("provider PNG header mismatch".into());
                }
                for (mode, expected) in [
                    (1, None),
                    (2, Some(FileRejection::Count)),
                    (3, Some(FileRejection::Type)),
                    (4, Some(FileRejection::Unavailable)),
                ] {
                    foreground().await?;
                    configure(mode)?;
                    // Keep a completed receipt alive: terminal cleanup must not
                    // depend on the caller dropping its Future after Ready.
                    let mut receipt = driver.pick(FileConfig::default());
                    let result = std::future::poll_fn(|cx| receipt.as_mut().poll(cx)).await;
                    if native(driver.stats())?.picker != 0 {
                        return Err("completed picker retained native receipt".into());
                    }
                    match (result, expected) {
                        (Ok(None), None) => (),
                        (Err(error), Some(expected)) if error == expected => (),
                        _ => return Err("native cancellation/refusal mismatch".into()),
                    }
                }
                for (config, expected) in [
                    (
                        FileConfig {
                            filters: vec![FileFilter::Extension(".txt".into())],
                            ..Default::default()
                        },
                        FileRejection::Type,
                    ),
                    (
                        FileConfig {
                            max_file_bytes: 1,
                            ..Default::default()
                        },
                        FileRejection::Bytes,
                    ),
                ] {
                    foreground().await?;
                    if !matches!(driver.pick(config).await,Err(error) if error==expected) {
                        return Err("provider filter/size refusal missing".into());
                    }
                }
                // Previous opened capability survives later cancelled/refused selections.
                if native(single[0].reader.read(0, 8).await)? != b"\x89PNG\r\n\x1a\n" {
                    return Err("prior handle changed".into());
                }
                drop(single);
                foreground().await?;
                let scope = weak.upgrade().ok_or("probe retired")?;
                let node = reference.get().ok_or("file node missing")?;
                scope.pick_files(node)?;
                drop(scope);
                let selected = take(selected).await?;
                if selected.len() != 2
                    || selected[0].name != "中文😀.txt"
                    || selected[1].name != "badge.png"
                {
                    return Err("semantic multi-selection mismatch".into());
                }
                let scope = weak.upgrade().ok_or("probe retired")?;
                let read = scope.cage(None);
                scope.request(
                    Query::FileRead(FileRead {
                        file: selected[0].token,
                        offset: 0,
                        length: selected[0].size as u32,
                    }),
                    move |result| read.revise(|v| *v = Some(result)),
                )?;
                drop(scope);
                match take(read).await?? {
                    QueryValue::FileBytes(chunk)
                        if chunk.bytes == "中文😀 owned file\n".as_bytes() => {}
                    _ => return Err("semantic provider read mismatch".into()),
                }
                // Warm the ordinary package badge before measuring a private preview.
                let deadline = Instant::now() + Duration::from_secs(10);
                while renderer.resource_stats().cached_images == 0 {
                    if Instant::now() > deadline {
                        return Err("package badge did not decode".into());
                    }
                    pause().await;
                }
                let baseline = renderer.resource_stats().cached_images;
                let scope = weak.upgrade().ok_or("probe retired")?;
                let response = scope.cage(None);
                scope.request(Query::FilePreview(selected[1].token), move |result| {
                    response.revise(|v| *v = Some(result))
                })?;
                drop(scope);
                match take(response).await?? {
                    QueryValue::FilePreview(Some(value)) => {
                        preview.revise(|v| *v = Some(value.source))
                    }
                    _ => return Err("semantic preview missing".into()),
                }
                let deadline = Instant::now() + Duration::from_secs(15);
                while renderer.resource_stats().cached_images <= baseline {
                    if Instant::now() > deadline {
                        return Err("private preview did not decode".into());
                    }
                    pause().await;
                }
                let scope = weak.upgrade().ok_or("probe retired")?;
                scope.clear_files(node)?;
                preview.revise(|v| *v = None);
                // The retired token cannot publish a later successful read.
                let retired = scope.cage(None);
                let requested = scope.request(
                    Query::FileRead(FileRead {
                        file: selected[0].token,
                        offset: 0,
                        length: 1,
                    }),
                    move |result| retired.revise(|v| *v = Some(result)),
                );
                drop(scope);
                if requested.is_ok() && take(retired).await?.is_ok() {
                    return Err("retired token retained read rights".into());
                }
                let deadline = Instant::now() + Duration::from_secs(10);
                loop {
                    let native = native(driver.stats())?;
                    let shared = files.stats();
                    if native.picker == 0
                        && native.read_records == 0
                        && native.handles == 0
                        && native.jobs == 0
                        && native.active_reads == 0
                        && native.metadata_bytes == 0
                        && native.declared_bytes == 0
                        && provider_handles()? == 0
                        && shared.references == 0
                        && shared.preview_bytes == 0
                        && renderer.resource_stats().cached_images == baseline
                    {
                        break;
                    }
                    if Instant::now() > deadline {
                        return Err("native/provider/preview grants did not drain".into());
                    }
                    pause().await;
                }
                Ok::<_, Box<dyn std::error::Error>>(())
            }
            .await;
            eprintln!("Glory Android files acceptance: {result:?}");
            status.revise(|v| {
                *v = if result.is_ok() {
                    "Activity results, ContentResolver reads, private decode and revocation passed"
                        .into()
                } else {
                    "Files acceptance failed; inspect body-free diagnostics".into()
                }
            });
        })?;
        Ok(())
    })
}
