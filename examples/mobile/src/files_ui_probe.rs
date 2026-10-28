//! Actual DocumentsUI acceptance. No selector replacement or selected-URI input.
use glory::{
    blitz::{
        Renderer,
        files::{AndroidFiles, FilePicker, Files},
    },
    prelude::*,
    render::{
        FileConfig, FileFilter, FileInfo, FileOutcome, FileRead, FileRejection, Query, QueryValue,
    },
    ui::WeakScope,
};
use jni::{JavaVM, jni_sig, jni_str, objects::JLongArray};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
type ProbeResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn native<T>(value: std::result::Result<T, FileRejection>) -> ProbeResult<T> {
    value.map_err(|error| std::io::Error::other(format!("native files: {error:?}")).into())
}
async fn pause() {
    futures_timer::Delay::new(Duration::from_millis(16)).await;
}
fn trace() -> ProbeResult<Vec<i64>> {
    JavaVM::singleton()?.attach_current_thread(|env| -> ProbeResult<_> {
        let class = env.load_class(jni_str!("rs.glory.counter.GlorySystemFilesTrace"))?;
        let value = env
            .call_static_method(class, jni_str!("snapshot"), jni_sig!(() -> [i64]), &[])?
            .l()?;
        let array = JLongArray::cast_local(env, value)?;
        if array.len(env)? != 8 {
            return Err("trace shape".into());
        }
        let mut values = vec![0; 8];
        array.get_region(env, 0, &mut values)?;
        Ok(values)
    })
}
async fn take<T: Clone + 'static>(value: Cage<Option<T>>) -> ProbeResult<T> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = value.get() {
            return Ok(value);
        }
        if Instant::now() > deadline {
            return Err("query deadline".into());
        }
        pause().await;
    }
}
async fn read(weak: &WeakScope, file: &FileInfo) -> ProbeResult {
    let expected: &[u8] = match file.name.as_str() {
        "glory-system-text.txt" => "Glory Android DocumentsUI: 中文😀\n".as_bytes(),
        "glory-system-image.png" => include_bytes!("../assets/badge.png"),
        _ => return Err("only owned system fixture files may be read".into()),
    };
    if file.size != expected.len() as u64 {
        return Err("fixture size mismatch".into());
    }
    let parent = weak.upgrade().ok_or("retired")?;
    let scope = parent.child()?;
    let reply = scope.cage(None);
    scope.request(
        Query::FileRead(FileRead {
            file: file.token,
            offset: 0,
            length: file.size as u32,
        }),
        move |value| reply.revise(|v| *v = Some(value)),
    )?;
    let result = take(reply).await;
    scope.owner().dispose();
    if !matches!(result??, QueryValue::FileBytes(value) if value.bytes == expected) {
        return Err("exact fixture read mismatch".into());
    }
    Ok(())
}
pub(crate) fn fixture(driver: Arc<AndroidFiles>, files: Files, renderer: Renderer) -> impl Widget {
    widget(move |scope| {
        let node = scope.cage(None);
        let outcome = scope.cage(None::<FileOutcome>);
        let status = scope.cage("Use storage → Download → glory-owned-system-files".to_owned());
        let clear = scope.cage(false);
        let preview = scope.cage(None::<String>);
        let choose = scope.downgrade();
        let retire_scope = scope.downgrade();
        let retire_driver = driver.clone();
        col().style(Style { display: Some(Display::Flex), flex_direction: Some(FlexDirection::Column), align: Some(Align::Start), font_size: Some(18.), ..Style::new().padding(Edges::all(24.)).gap(12.) })
            .fill((
                text().content("Android System Files Acceptance"), text().content(status),
                glory::ui::file().label("Choose system documents").node_ref(node)
                    .configuration(FileConfig { multiple: true, max_files: 2, filters: vec![FileFilter::Extension(".txt".into()), FileFilter::Extension(".png".into())], preview: Some(Default::default()), ..Default::default() })
                    .on(activate, move |_| {
                        if let Some(scope) = choose.upgrade() && let Some(node) = node.get()
                            && let Err(error) = scope.pick_files(node) { scope.report_error(error); }
                    })
                    .on(glory::render::EventKind::Files, move |event| {
                        if let Event::Files(event) = event { outcome.revise(|v| *v = Some(event.outcome)); }
                    }),
                button().label("Clear and verify release").on(activate, move |_| clear.revise(|v| *v = true)),
                button().label("Open then retire requester").on(activate, move |_| {
                    let Some(scope) = retire_scope.upgrade() else { return; };
                    let owner = scope.owner().child(); let called = std::rc::Rc::new(std::cell::Cell::new(false));
                    let callback = called.clone(); let driver = retire_driver.clone(); let before = trace();
                    let future = driver.pick(FileConfig::default());
                    if let Err(error) = owner.spawn(async move { let _ = future.await; callback.set(true); }) { scope.report_error(error); return; }
                    let driver = retire_driver.clone();
                    status.revise(|v| *v = "Requester retires in 3 seconds; then return a result in DocumentsUI".into());
                    let result = scope.owner().spawn(async move {
                        let result = async {
                            let before = before?;
                            futures_timer::Delay::new(Duration::from_secs(3)).await; owner.dispose();
                            if trace()?[0] <= before[0] || native(driver.stats())?.picker != 1 || called.get() {
                                return Err("real picker not retained on retirement".into());
                            }
                            if !matches!(driver.pick(FileConfig::default()).await, Err(FileRejection::Busy)) { return Err("retirement refunded active system picker".into()); }
                            eprintln!("Glory DocumentsUI retired pending: permit retained, second picker busy");
                            let deadline = Instant::now() + Duration::from_secs(90);
                            while native(driver.stats())?.picker != 0 {
                                if Instant::now() > deadline { return Err("return a system result after retirement".into()); }
                                pause().await;
                            }
                            let after = trace()?;
                            if called.get() || after[7] <= before[7] || after[3] != before[3] { return Err("late result reached retired requester or opened a descriptor".into()); }
                            eprintln!("Glory DocumentsUI retired passed trace={after:?}");
                            status.revise(|v| *v = "Retired requester: system result ignored, permit released".into());
                            Ok::<_, Box<dyn std::error::Error>>(())
                        }.await;
                        if let Err(error) = result { eprintln!("Glory DocumentsUI failure retirement: {error}"); }
                    });
                    if let Err(error) = result { scope.report_error(error); }
                }),
                when(preview.map(|v| v.is_some()), move || image().source(preview.get().expect("live preview")).decorative().style(Style::new().width(Length::Px(64.)).height(Length::Px(64.)))),
            )).show_in(scope)?;
        let weak = scope.downgrade();
        let driver = driver.clone();
        let files = files.clone();
        let renderer = renderer.clone();
        scope.owner().spawn(async move {
            let mut grants = Vec::<FileInfo>::new(); let mut sequence = 0; let mut baseline = None;
            loop {
                let next = outcome.get();
                if next.is_none() && !clear.get() { pause().await; continue; }
                outcome.revise(|v| *v = None); sequence += 1;
                let result = async {
                    if clear.get() {
                        clear.revise(|v| *v = false); let scope = weak.upgrade().ok_or("retired")?;
                        let retired = grants.first().map(|file| file.token);
                        scope.clear_files(node.get().ok_or("file node")?)?; preview.revise(|v| *v = None); grants.clear();
                        if let Some(token) = retired {
                            let query = scope.child()?; let reply = query.cage(None);
                            let requested = query.request(Query::FileRead(FileRead { file: token, offset: 0, length: 1 }), move |value| reply.revise(|v| *v = Some(value)));
                            let result = if requested.is_ok() { Some(take(reply).await) } else { None };
                            query.owner().dispose();
                            if let Some(result) = result && result?.is_ok() { return Err("retired token remained readable".into()); }
                        }
                        let deadline = Instant::now() + Duration::from_secs(15);
                        loop {
                            let stats = native(driver.stats())?; let shared = files.stats();
                            if stats.picker == 0 && stats.handles == 0 && stats.read_records == 0 && stats.jobs == 0 && stats.active_reads == 0 && stats.metadata_bytes == 0 && stats.declared_bytes == 0
                                && shared.references == 0 && shared.preview_bytes == 0 && baseline.is_none_or(|b| renderer.resource_stats().cached_images == b) { break; }
                            if Instant::now() > deadline { return Err("native/shared/preview cleanup did not settle".into()); } pause().await;
                        }
                        let t = trace()?; if t[3] != t[4] { return Err("descriptor closes unbalanced".into()); }
                        eprintln!("Glory DocumentsUI cleared seq={sequence} trace={t:?}");
                        status.revise(|v| *v = "Clear: native grants, descriptors and private preview released".into()); return Ok(());
                    }
                    match next.ok_or("missing outcome")? {
                        FileOutcome::Selected(selected) => {
                            preview.revise(|v| *v = None);
                            for file in &selected { read(&weak, file).await?; }
                            let t = trace()?; if t[5] < selected.len() as i64 { return Err("no system document provider grants".into()); }
                            if let Some(file) = selected.iter().find(|v| v.name.ends_with(".png")) {
                                let deadline = Instant::now() + Duration::from_secs(15);
                                while renderer.resource_stats().cached_images == 0 { if Instant::now() > deadline { return Err("package image not ready".into()); } pause().await; }
                                let base = *baseline.get_or_insert(renderer.resource_stats().cached_images);
                                while renderer.resource_stats().cached_images > base { if Instant::now() > deadline { return Err("old preview cache not retired".into()); } pause().await; }
                                let parent = weak.upgrade().ok_or("retired")?; let scope = parent.child()?; let reply = scope.cage(None);
                                scope.request(Query::FilePreview(file.token), move |value| reply.revise(|v| *v = Some(value)))?;
                                let result = take(reply).await; scope.owner().dispose();
                                match result?? { QueryValue::FilePreview(Some(value)) => preview.revise(|v| *v = Some(value.source)), _ => return Err("private preview missing".into()) }
                                let deadline = Instant::now() + Duration::from_secs(15);
                                while renderer.resource_stats().cached_images <= base { if Instant::now() > deadline { return Err("system PNG did not decode".into()); } pause().await; }
                            }
                            grants = selected;
                            eprintln!("Glory DocumentsUI selected seq={sequence} count={} trace={:?}", grants.len(), trace()?);
                            status.revise(|v| *v = format!("Selected {} system files: exact reads and preview passed", grants.len()));
                        }
                        FileOutcome::Cancelled => {
                            for file in &grants { read(&weak, file).await?; }
                            eprintln!("Glory DocumentsUI cancelled seq={sequence} preserved={} trace={:?}", grants.len(), trace()?);
                            status.revise(|v| *v = format!("System Cancel passed; {} prior grants preserved", grants.len()));
                        }
                        FileOutcome::Rejected(FileRejection::Count) => {
                            for file in &grants { read(&weak, file).await?; }
                            eprintln!("Glory DocumentsUI count refused seq={sequence} preserved={} trace={:?}", grants.len(), trace()?);
                            status.revise(|v| *v = format!("Count refusal passed; {} prior grants preserved", grants.len()));
                        }
                        FileOutcome::Rejected(error) => return Err(format!("system selection rejected: {error:?}").into()),
                    }
                    Ok::<_, Box<dyn std::error::Error>>(())
                }.await;
                if let Err(error) = result { eprintln!("Glory DocumentsUI failure seq={sequence}: {error}"); status.revise(|v| *v = "System files failed; see body-free log".into()); }
            }
        })?;
        Ok(())
    })
}
