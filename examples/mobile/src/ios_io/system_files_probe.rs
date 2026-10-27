//! Real system picker acceptance. No selected-URL injection or shared container.
use super::IosFiles;
use glory::ui::WeakScope;
use glory::{
    blitz::{
        Renderer,
        files::{FilePicker, Files},
    },
    prelude::*,
    render::{FileConfig, FileFilter, FileInfo, FileOutcome, FileRead, Query, QueryValue},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
type ProbeResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
async fn pause() {
    futures_timer::Delay::new(Duration::from_millis(16)).await;
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
        "glory-provider-text.txt" => "Glory external provider: 中文😀\n".as_bytes(),
        "glory-provider-image.png" => include_bytes!("../../assets/badge.png"),
        _ => return Err("only owned provider fixture files are accepted".into()),
    };
    if file.size != expected.len() as u64 {
        return Err("fixture size mismatch".into());
    }
    let parent = weak.upgrade().ok_or("retired")?;
    let scope = parent.child()?;
    let value = scope.cage(None);
    scope.request(
        Query::FileRead(FileRead {
            file: file.token,
            offset: 0,
            length: file.size as u32,
        }),
        move |reply| value.revise(|v| *v = Some(reply)),
    )?;
    let result = take(value).await;
    scope.owner().dispose();
    if !matches!(result??,QueryValue::FileBytes(chunk) if chunk.bytes==expected) {
        return Err("external scoped read mismatch".into());
    }
    Ok(())
}
pub(crate) fn fixture(driver: Arc<IosFiles>, files: Files, renderer: Renderer) -> impl Widget {
    widget(move |scope| {
        let node = scope.cage(None);
        let status = scope.cage("Use real Files UI to select Glory Provider fixtures".to_owned());
        let outcome = scope.cage(None::<FileOutcome>);
        let clear = scope.cage(false);
        let preview = scope.cage(None::<String>);
        let cleanup = driver.clone();
        scope.owner().on_cleanup(move || cleanup.close())?;
        let choose = scope.downgrade();
        let cancel_owner = scope.downgrade();
        let cancel_driver = driver.clone();
        col()
            .style(Style { display: Some(Display::Flex), flex_direction: Some(FlexDirection::Column), align: Some(Align::Start), font_size: Some(18.), ..Style::new().padding(Edges::all(24.)).gap(12.) })
            .fill((
                text().content("iOS System Files Acceptance"),
                text().content(status),
                glory::ui::file()
                    .label("Choose provider documents")
                    .node_ref(node)
                    .configuration(FileConfig {
                        multiple: true,
                        max_files: 2,
                        filters: vec![
                            FileFilter::Extension(".txt".into()),
                            FileFilter::Extension(".png".into()),
                        ],
                        preview: Some(Default::default()),
                        ..Default::default()
                    })
                    .on(activate, move |_| {
                        if let Some(scope) = choose.upgrade() && let Some(node) = node.get()
                            && let Err(error) = scope.pick_files(node) { scope.report_error(error); }
                    })
                    .on(glory::render::EventKind::Files, move |event| {
                        if let Event::Files(event) = event {
                            outcome.revise(|v| *v = Some(event.outcome));
                        }
                    }),
                button()
                    .label("Clear and verify release")
                    .on(activate, move |_| clear.revise(|v| *v = true)),
                button().label("Open then retire requester").on(activate, move |_| {
                    let Some(scope) = cancel_owner.upgrade() else { return; };
                    let owner = scope.owner().child();
                    let called = std::rc::Rc::new(std::cell::Cell::new(false));
                    let callback = called.clone();
                    let driver = cancel_driver.clone();
                    let before = driver.trace();
                    let future = driver.pick(FileConfig::default());
                    if let Err(error) = owner.spawn(async move { let _ = future.await; callback.set(true); }) { scope.report_error(error); return; }
                    status.revise(|v| *v = "Real picker opens; requester retires in five seconds".into());
                    let driver = cancel_driver.clone();
                    let result = scope.owner().spawn(async move {
                        futures_timer::Delay::new(Duration::from_secs(5)).await;
                        owner.dispose();
                        let deadline = Instant::now() + Duration::from_secs(8);
                        loop {
                            if driver.stats().is_ok_and(|stats| stats[0] == 0 && stats[1] == 0) { break; }
                            if Instant::now() > deadline { eprintln!("Glory system picker owner cancellation failed: unsettled picker"); return; }
                            pause().await;
                        }
                        let trace = driver.trace();
                        if called.get() || trace[0] <= before[0] || trace[9] <= before[9] {
                            eprintln!("Glory system picker owner cancellation failed: completion/trace"); return;
                        }
                        eprintln!("Glory system picker owner cancellation passed trace={trace:?}");
                        status.revise(|v| *v = "Retired requester: actual sheet dismissed; no late callback".into());
                    });
                    if let Err(error) = result { scope.report_error(error); }
                }),
                when(preview.map(|v| v.is_some()), move || {
                    image()
                        .source(preview.get().expect("live preview"))
                        .decorative()
                        .style(Style::new().width(Length::Px(64.)).height(Length::Px(64.)))
                }),
            ))
            .show_in(scope)?;
        let weak = scope.downgrade();
        let driver = driver.clone();
        let files = files.clone();
        let renderer = renderer.clone();
        scope.owner().spawn(async move{
            let mut granted:Vec<FileInfo>=Vec::new();let mut sequence=0;let mut baseline=0;
            loop{
                let next=outcome.get();
                if next.is_none() && !clear.get(){pause().await;continue;}
                outcome.revise(|v|*v=None);sequence+=1;
                let result=async{
                    if clear.get(){
                        clear.revise(|v|*v=false);let scope=weak.upgrade().ok_or("retired")?;scope.clear_files(node.get().ok_or("file node")?)?;preview.revise(|v|*v=None);granted.clear();
                        let deadline=Instant::now()+Duration::from_secs(10);
                        loop{
                            let stats=driver.stats().map_err(|e|std::io::Error::other(format!("native stats: {e:?}")))?;
                            if stats[..6]==[0;6] && files.stats().references==0 && files.stats().preview_bytes==0 && (baseline==0 || renderer.resource_stats().cached_images==baseline){break;}
                            if Instant::now()>deadline{return Err("native grants did not close".into());}pause().await;
                        }
                        let trace=driver.trace();if trace[3]!=trace[4] || trace[5]!=trace[6] || trace[8]!=0{return Err("security scopes or handles unbalanced".into());}
                        eprintln!("Glory system files cleared seq={sequence} native={:?} trace={trace:?}",driver.stats());status.revise(|v|*v="Clear: scopes, handles and private preview released".into());return Ok(());
                    }
                    match next.ok_or("missing outcome")?{
                        FileOutcome::Selected(selected)=>{
                            preview.revise(|v|*v=None);
                            for file in &selected{read(&weak,file).await?;}
                            let trace=driver.trace();if trace[7]<selected.len() as i64 || trace[8]!=0{return Err("no external security-scoped provider open".into());}
                            if let Some(file)=selected.iter().find(|file|file.name.ends_with(".png")){
                                let deadline=Instant::now()+Duration::from_secs(15);while renderer.resource_stats().cached_images==0{if Instant::now()>deadline{return Err("package image not ready".into());}pause().await;}
                                baseline=renderer.resource_stats().cached_images;
                                let parent=weak.upgrade().ok_or("retired")?;let scope=parent.child()?;let value=scope.cage(None);scope.request(Query::FilePreview(file.token),move|reply|value.revise(|v|*v=Some(reply)))?;
                                let result=take(value).await;scope.owner().dispose();
                                match result??{QueryValue::FilePreview(Some(value))=>preview.revise(|v|*v=Some(value.source)),_=>return Err("private preview missing".into())}
                                let deadline=Instant::now()+Duration::from_secs(15);while renderer.resource_stats().cached_images<=baseline{if Instant::now()>deadline{return Err("external PNG did not decode".into());}pause().await;}
                            }else{baseline=renderer.resource_stats().cached_images;}
                            granted=selected;eprintln!("Glory system files selected seq={sequence} count={} trace={:?}",granted.len(),driver.trace());status.revise(|v|*v=format!("Selected {} external files: exact reads and preview passed",granted.len()));
                        }
                        FileOutcome::Cancelled=>{for file in &granted{read(&weak,file).await?;}eprintln!("Glory system files cancelled seq={sequence} preserved={} trace={:?}",granted.len(),driver.trace());status.revise(|v|*v=format!("System Cancel passed; {} prior grants preserved",granted.len()));}
                        FileOutcome::Rejected(_)=>return Err("system selection rejected".into()),
                    }Ok::<_,Box<dyn std::error::Error>>(())
                }.await;
                if result.is_err(){eprintln!("Glory system files failure seq={sequence}: {result:?}");status.revise(|v|*v="System files failed; inspect body-free log".into());}
            }
        })?;
        Ok(())
    })
}
