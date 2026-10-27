//! Owned UIKit/service acceptance. No system-picker or paste gesture injection.
use super::{IosClipboard, IosFiles};
use glory::{
    blitz::{
        Renderer,
        clipboard::{ClipboardDriver, ClipboardError},
        files::{FilePicker, Files},
    },
    prelude::*,
    render::{
        ClipboardOutcome, ClipboardText, FileConfig, FileFilter, FileInfo, FileOutcome, FileRead,
        FileRejection, Query, QueryValue, Semantics,
    },
};
use std::{
    sync::Arc,
    task::{Context, Waker},
    time::{Duration, Instant},
};
type ProbeResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn native<T>(value: std::result::Result<T, FileRejection>) -> ProbeResult<T> {
    value.map_err(|e| std::io::Error::other(format!("iOS file service: {e:?}")).into())
}
async fn pause() {
    futures_timer::Delay::new(Duration::from_millis(8)).await;
}
async fn take<T: Clone + 'static>(value: Cage<Option<T>>) -> ProbeResult<T> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(value) = value.get() {
            return Ok(value);
        }
        if Instant::now() > deadline {
            return Err("semantic reply deadline".into());
        }
        pause().await;
    }
}
fn configuration() -> FileConfig {
    FileConfig {
        multiple: true,
        max_files: 2,
        filters: vec![
            FileFilter::Extension(".txt".into()),
            FileFilter::Extension(".png".into()),
        ],
        preview: Some(Default::default()),
        ..Default::default()
    }
}
pub(crate) fn fixture(
    clipboard: Arc<IosClipboard>,
    driver: Arc<IosFiles>,
    files: Files,
    renderer: Renderer,
) -> impl Widget {
    widget(move |scope| {
        let status = scope.cage("Waiting for owned iOS IO guard".to_owned());
        let copy_ref = scope.cage(None);
        let file_ref = scope.cage(None);
        let copied = scope.cage(None::<ClipboardOutcome>);
        let selected = scope.cage(None::<Vec<FileInfo>>);
        let preview = scope.cage(None::<String>);
        let cleanup_clipboard = clipboard.clone();
        let cleanup_files = driver.clone();
        scope.owner().on_cleanup(move || {
            let _ = cleanup_clipboard.probe(1);
            cleanup_clipboard.close();
            let _ = cleanup_files.probe(11);
            cleanup_files.close();
        })?;
        col()
            .style(Style::new().padding(Edges::all(24.)).gap(12.))
            .fill((
                text().content("iOS Files and Clipboard Probe"),
                text().content(status),
                button()
                    .label("Copy owned text")
                    .node_ref(copy_ref)
                    .semantics(Some(Box::new(Semantics {
                        clipboard: Some(ClipboardText {
                            text: String::new(),
                            revision: 0,
                        }),
                        ..Default::default()
                    })))
                    .on(glory::render::EventKind::Clipboard, move |event| {
                        if let Event::Clipboard { outcome, .. } = event {
                            copied.revise(|v| *v = Some(outcome));
                        }
                    }),
                glory::ui::file()
                    .label("Choose owned documents")
                    .node_ref(file_ref)
                    .configuration(configuration())
                    .on(glory::render::EventKind::Files, move |event| {
                        if let Event::Files(event) = event
                            && let FileOutcome::Selected(value) = event.outcome
                        {
                            selected.revise(|v| *v = Some(value));
                        }
                    }),
                input().name("Clipboard editor").value("A中😀Z"),
                when(preview.map(|v| v.is_some()), move || {
                    image()
                        .source(preview.get().expect("live preview"))
                        .decorative()
                        .style(Style::new().width(Length::Px(64.)).height(Length::Px(64.)))
                }),
            ))
            .show_in(scope)?;
        let weak = scope.downgrade();
        let clipboard = clipboard.clone();
        let driver = driver.clone();
        let files = files.clone();
        let renderer = renderer.clone();
        scope.owner().spawn(async move {
            let result=async {
                let deadline=Instant::now()+Duration::from_secs(15);
                while clipboard.probe(0)!=1 { if Instant::now()>deadline {return Err("owned pasteboard guard unavailable".into());} pause().await; }
                super::audit_integer_abi();
                let original_owned=clipboard.read(65536).await?;
                if !original_owned.starts_with("glory-owned-ios-clipboard-") {return Err("owned clipboard guard mismatch".into());}
                let text=format!("{original_owned}:中😀\r\nsecond\0");
                let expected=glory::render::normalize_text(&text,true).into_owned();
                drop(clipboard.write(text.clone()));
                if clipboard.stats()? != (0,0) {return Err("lazy clipboard construction admitted native work".into());}
                clipboard.write(text).await?;
                if clipboard.read(65536).await? != expected {return Err("clipboard round trip mismatch".into());}
                if !matches!(clipboard.read(8).await,Err(ClipboardError::TooLong)) || !matches!(clipboard.write("x".repeat(65537)).await,Err(ClipboardError::TooLong)) {return Err("clipboard budgets missing".into());}
                let scope=weak.upgrade().ok_or("probe retired")?; let copy=copy_ref.get().ok_or("copy node absent")?;
                scope.patch(copy,glory::render::Patch::Semantics(glory::render::Change::Set(Some(Box::new(Semantics{clipboard:Some(ClipboardText{text:expected.clone(),revision:1}),..Default::default()})))))?;
                scope.copy_text(copy)?; drop(scope);
                if take(copied).await? != ClipboardOutcome::Written || clipboard.read(65536).await? != expected {return Err("semantic clipboard copy failed".into());}
                let config=configuration();
                drop(driver.pick(config.clone()));
                if native(driver.stats())?[1]!=0 {return Err("lazy picker construction admitted native work".into());}
                native(driver.probe(1))?;
                let direct=native(driver.pick(config.clone()).await)?.ok_or("owned selection missing")?;
                if direct.len()!=2 || direct[0].name!="中文😀.txt" || direct[1].media_type!="image/png" {return Err("native file metadata mismatch".into());}
                if native(direct[0].reader.read(0,9).await)? != "A中😀Z".as_bytes() || native(direct[1].reader.read(0,8).await)? != b"\x89PNG\r\n\x1a\n" {return Err("coordinated native read mismatch".into());}
                for (mode,config,expected) in [(2,config.clone(),None),(3,config.clone(),Some(FileRejection::Count)),(4,FileConfig{filters:vec![FileFilter::Extension(".txt".into())],..Default::default()},Some(FileRejection::Type))] {
                    native(driver.probe(mode))?; let result=driver.pick(config).await;
                    match (result,expected) { (Ok(None),None)=>(),(Err(a),Some(b)) if a==b=>(),_=>return Err("picker cancellation/refusal mismatch".into()) }
                }
                // Hold real worker reads to prove cancellation cannot refund
                // physical admission or the last selected handle/scope.
                native(driver.probe(10))?;
                let mut pending:Vec<_>=(0..4).map(|_|direct[0].reader.read(0,9)).collect();
                for future in &mut pending {if !future.as_mut().poll(&mut Context::from_waker(Waker::noop())).is_pending() {return Err("read gate did not hold actual operation".into());}}
                drop(pending.drain(..2));
                if native(driver.stats())?[2]!=4 {return Err("cancel refunded real reads".into());}
                if !matches!(direct[0].reader.read(0,9).await,Err(FileRejection::Busy)) {return Err("fifth native read bypassed admission".into());}
                native(driver.probe(11))?;
                for future in pending {if native(future.await)? != "A中😀Z".as_bytes() {return Err("live read after cancellation mismatch".into());}}
                let deadline = Instant::now()+Duration::from_secs(10);
                while native(driver.stats())?[2] != 0 { if Instant::now()>deadline {return Err("cancelled reads did not settle".into());} pause().await; }
                native(driver.probe(10))?;
                let executor = std::rc::Rc::new(glory::core::executor::Executor::new(2));
                let runtime = glory::core::Runtime::with_host(executor.clone());
                let owner = runtime.owner();
                let late = std::rc::Rc::new(std::cell::Cell::new(false));
                let callback = late.clone();
                let future = direct[0].reader.read(0,9);
                owner.spawn(async move { let _ = future.await; callback.set(true); })?;
                if !executor.poll_one() {return Err("Owner read was not polled".into());}
                owner.dispose();
                drop(direct);
                let physical = native(driver.stats())?;
                if runtime.stats().tasks != 0 || physical[2] != 1 || physical[3] == 0 {return Err("Owner cancellation refunded actual read/last handle".into());}
                native(driver.probe(11))?;
                let deadline=Instant::now()+Duration::from_secs(10);
                while native(driver.stats())?[2..6] != [0;4] {if Instant::now()>deadline{return Err("last actual read/close did not settle".into());}pause().await;}
                if executor.poll_one() || late.get(){return Err("late Owner file delivery".into());}
                eprintln!("Glory iOS IO admission passed: four held reads, fifth rejected, cancelled receipts charged, retired Owner silent, last actual read/close released quota");
                // Ordinary semantic selection/read/private preview pipeline.
                native(driver.probe(1))?;
                let scope=weak.upgrade().ok_or("probe retired")?; let node=file_ref.get().ok_or("file node absent")?; scope.pick_files(node)?; drop(scope);
                let selected=take(selected).await?;
                if selected.len()!=2 {return Err("semantic multi-selection missing".into());}
                let scope=weak.upgrade().ok_or("probe retired")?; let response=scope.cage(None);
                scope.request(Query::FileRead(FileRead{file:selected[0].token,offset:0,length:9}),move|v|response.revise(|slot|*slot=Some(v)))?; drop(scope);
                if !matches!(take(response).await??,QueryValue::FileBytes(chunk) if chunk.bytes=="A中😀Z".as_bytes()) {return Err("semantic file read mismatch".into());}
                let deadline=Instant::now()+Duration::from_secs(15);
                while renderer.resource_stats().cached_images==0 {if Instant::now()>deadline{return Err("package badge did not decode".into());}pause().await;}
                let baseline=renderer.resource_stats().cached_images;
                let scope=weak.upgrade().ok_or("probe retired")?; let response=scope.cage(None);
                scope.request(Query::FilePreview(selected[1].token),move|v|response.revise(|slot|*slot=Some(v)))?;drop(scope);
                match take(response).await?? {QueryValue::FilePreview(Some(value))=>preview.revise(|v|*v=Some(value.source)),_=>return Err("private preview missing".into())}
                let deadline=Instant::now()+Duration::from_secs(15);
                while renderer.resource_stats().cached_images<=baseline {if Instant::now()>deadline{return Err("private preview did not decode".into());}pause().await;}
                let scope=weak.upgrade().ok_or("probe retired")?;scope.clear_files(node)?;preview.revise(|v|*v=None);
                let response=scope.cage(None);let retired=scope.request(Query::FileRead(FileRead{file:selected[0].token,offset:0,length:1}),move|v|response.revise(|slot|*slot=Some(v)));drop(scope);
                if retired.is_ok() && take(response).await?.is_ok(){return Err("revoked file token retained read rights".into());}
                let deadline=Instant::now()+Duration::from_secs(15);
                loop {
                    let native=native(driver.stats())?;let shared=files.stats();
                    if native[..6]==[0;6] && shared.references==0 && shared.reads==0 && shared.preview_bytes==0 && renderer.resource_stats().cached_images==baseline {break;}
                    if Instant::now()>deadline{return Err("native handles/scopes/preview grants did not drain".into());}pause().await;
                }
                Ok::<_,Box<dyn std::error::Error>>(())
            }.await;
            // Always unblock owned workers and revoke semantic files on error.
            let _=driver.probe(11);
            if let Some(scope)=weak.upgrade(){if let Some(node)=file_ref.get(){let _=scope.clear_files(node);}preview.revise(|v|*v=None);}
            let restored=async {
                let deadline=Instant::now()+Duration::from_secs(10);
                let _=clipboard.probe(1);
                loop {match clipboard.probe(0){2=>return Ok::<_,Box<dyn std::error::Error>>(()),0 if Instant::now()<deadline=>pause().await,_=>return Err("pasteboard restore failed or external change preserved".into())}}
            }.await;
            let result=result.and(restored);
            eprintln!("Glory iOS IO final stats: native_files={:?}; shared_files={:?}; native_clipboard={:?}; guard={}",driver.stats(),files.stats(),clipboard.stats(),clipboard.probe(0));
            eprintln!("Glory iOS files and clipboard acceptance: {result:?}");
            status.revise(|v|*v=if result.is_ok(){"Coordinated file reads, private decode, cancellation, clipboard and restoration passed".into()}else{"iOS IO acceptance failed; inspect body-free diagnostics".into()});
        })?;
        Ok(())
    })
}
