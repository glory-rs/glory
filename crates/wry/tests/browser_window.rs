//! Real native reader isolation/history acceptance against an owned HTTP fixture.
use glory_wry::shell::{browser::{Controller,Action},*};
use std::{io::{Read,Write},sync::{Arc,atomic::{AtomicBool,AtomicUsize,Ordering}},time::{Duration,Instant}};
fn main()->Result<(),Box<dyn std::error::Error>> {
    if std::env::var("GLORY_WRY_BROWSER").as_deref()!=Ok("1"){println!("Set GLORY_WRY_BROWSER=1 for native reader acceptance");return Ok(());}
    let listener=std::net::TcpListener::bind("127.0.0.1:0")?;listener.set_nonblocking(true)?;
    let address=listener.local_addr()?;let stop=Arc::new(AtomicBool::new(false));let worker_stop=stop.clone();
    let hits=Arc::new(AtomicUsize::new(0));let worker_hits=hits.clone();
    let worker=std::thread::spawn(move||{
        let deadline=Instant::now()+Duration::from_secs(25);
        while !worker_stop.load(Ordering::Acquire)&&Instant::now()<deadline {
            let Ok((mut stream,_))=listener.accept()else{std::thread::sleep(Duration::from_millis(5));continue};
            stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();let mut buffer=[0;8192];let read=stream.read(&mut buffer).unwrap_or(0);
            let two=String::from_utf8_lossy(&buffer[..read]).starts_with("GET /two ");
            if two{worker_hits.fetch_add(1,Ordering::AcqRel);}
            let script=if two{"document.title=(window.ipc||window.__gloryWryReceive)?'FAIL-bridge':'PageTwo-no-bridge';"}else{"document.title=(window.ipc||window.__gloryWryReceive)?'FAIL-bridge':'PageOne-no-bridge';if(!sessionStorage.visited){sessionStorage.visited='1';setTimeout(()=>location.href='/two',500);}"};
            let body=format!("<!doctype html><html><head><meta charset='utf-8'></head><body><h1>Owned native browser fixture</h1><script>{script}</script></body></html>");
            let response=format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());let _=stream.write_all(response.as_bytes());
        }
    });
    let controller=Controller::default();controller.open("owned",&format!("http://{address}/one"),"Owned reader")?;
    let app=WindowApp::new();app.mount(glory_ui::text().content("Native reader acceptance"))?;
    let start=Instant::now();let mut phase=0;let mut reload_hits=0;
    let result=Shell::new().run_until(app,Config {title:"Glory reader host acceptance".into(),browser:Some(controller.clone()),..Default::default()},|_|{
        if start.elapsed()>Duration::from_secs(20){return Err(ShellError(format!("Native reader timeout, phase {phase}, {:?}",controller.snapshots())));}
        let entries=controller.snapshots();let Some(page)=entries.first()else{return Ok(phase==4)};
        if page.title.starts_with("FAIL")||page.error.is_some(){return Err(ShellError("Untrusted page received a privileged bridge or native reader failed".into()));}
        match phase {
            0 if page.title=="PageTwo-no-bridge" && !page.loading=>{controller.action("owned",Action::Back)?;phase=1;},
            1 if page.title=="PageOne-no-bridge" && !page.loading=>{controller.action("owned",Action::Forward)?;phase=2;},
            2 if page.title=="PageTwo-no-bridge" && !page.loading=>{reload_hits=hits.load(Ordering::Acquire);controller.action("owned",Action::Reload)?;phase=3;},
            3 if hits.load(Ordering::Acquire)>reload_hits && page.title=="PageTwo-no-bridge" && !page.loading=>{controller.close_all();phase=4;},
            _=>{},
        }
        Ok(false)
    });
    stop.store(true,Ordering::Release);worker.join().unwrap();result?;
    println!("Native isolated reader, HTTP navigation, back/forward/reload and scoped close passed");Ok(())
}
