#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod desktop {
    //! Native media acceptance without autoplay or audio hardware access.
    use glory_ui::{events::*, *};
    use glory_wry::{LocalMedia, MediaKind, shell::*};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    struct Wav {
        bytes: Vec<u8>,
        offsets: Mutex<Vec<u64>>,
        mime: &'static str,
    }
    impl AssetSource for Wav {
        fn len(&self) -> u64 {
            self.bytes.len() as u64
        }
        fn mime(&self) -> &str {
            self.mime
        }
        fn current(&self) -> bool {
            true
        }
        fn read_range(&self, offset: u64, length: usize) -> std::result::Result<Vec<u8>, String> {
            assert!(length <= MAX_RANGE_BYTES);
            self.offsets.lock().unwrap().push(offset);
            Ok(self.bytes[offset as usize..offset as usize + length].to_vec())
        }
    }
    struct Provider(Arc<Wav>, Arc<Wav>);
    impl AssetProvider for Provider {
        fn get(&self, path: &str) -> Option<Arc<dyn AssetSource>> {
            match path {
                "/media/owned.wav" => Some(self.0.clone() as Arc<dyn AssetSource>),
                "/media/owned.png" => Some(self.1.clone() as Arc<dyn AssetSource>),
                _ => None,
            }
        }
    }
    pub(super) fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
        if std::env::var("GLORY_WRY_MEDIA").as_deref() != Ok("1") {
            println!("Set GLORY_WRY_MEDIA=1 for native media acceptance");
            return Ok(());
        }
        let size = 120 * 8000 * 2u32;
        let mut bytes = b"RIFF".to_vec();
        bytes.extend((size + 36).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(8000u32.to_le_bytes());
        bytes.extend(16000u32.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(size.to_le_bytes());
        bytes.resize(size as usize + 44, 0);
        let wav = Arc::new(Wav {
            bytes,
            offsets: Mutex::new(vec![]),
            mime: "audio/wav",
        });
        let png = Arc::new(Wav {
            bytes: vec![
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0,
                1, 8, 4, 0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252,
                255, 31, 0, 3, 3, 2, 0, 239, 154, 238, 95, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
                130,
            ],
            offsets: Mutex::new(vec![]),
            mime: "image/png",
        });
        let app = WindowApp::new();
        let scope = app.app().scope();
        let node = scope.cage(None);
        let image_node = scope.cage(None);
        let status = Rc::new(Cell::new(0));
        let receipt = status.clone();
        let part = Rc::new(RefCell::new(Some(
            group()
                .fill((
                    view().node_ref(node).class("player"),
                    view().node_ref(image_node).class("image"),
                ))
                .mount(scope)?,
        )));
        app.mount(col().fill((
            text().content("Owned media"),
            input().class("status").on(value_changed, move |event| {
                if let Event::ValueChanged { value, .. } = event {
                    receipt.set(value.parse().unwrap_or(-1));
                }
            }),
        )))?;
        app.app().flush()?;
        app.renderer().enable_local_media();
        app.renderer().set_local_media(
            node.get().unwrap(),
            Some(LocalMedia {
                kind: MediaKind::Audio,
                path: "/media/owned.wav",
                title: "Owned wave",
            }),
        )?;
        app.renderer().set_local_media(
            image_node.get().unwrap(),
            Some(LocalMedia {
                kind: MediaKind::Image,
                path: "/media/owned.png",
                title: "Owned image",
            }),
        )?;
        let started = Instant::now();
        let retired = Cell::new(0);
        Shell::new().run_until(app,Config {
        title:"Glory native media acceptance".into(),asset_provider:Some(Arc::new(Provider(wav.clone(),png.clone()))),
        initialization_script:Some(r#"(()=>{
          let phase=0, player=null, image=null;const start=Date.now();
          const timer=setInterval(()=>{try {
            const input=document.querySelector('.status input')||document.querySelector('input.status');
            const report=n=>{input.value=String(n);input.dispatchEvent(new Event('input',{bubbles:true}));};
            if(!input)return;
            if(phase===0) {player=document.querySelector('audio');if(!player||player.readyState<1)return;image=document.querySelector('.glory-image-viewer img');if(!image||!image.complete||!image.naturalWidth)return;
              document.querySelector('[aria-label="Zoom in"]').click();if(Number(image.dataset.zoom)<=1)throw Error('image zoom');document.querySelector('[aria-label="Rotate image"]').click();if(image.dataset.rotation!=='90')throw Error('image rotation');document.querySelector('[aria-label="Fit image"]').click();if(image.dataset.zoom!=='1'||image.dataset.rotation!=='0')throw Error('image fit');
              if(Math.abs(player.duration-120)>.1||!player.controls||!player.paused||player.autoplay)throw Error('metadata/controls');
              player.currentTime=100;phase=1;
            } else if(phase===1 && !player.seeking && Math.abs(player.currentTime-100)<1){report(1);phase=2;}
            else if(phase===2 && !document.querySelector('audio')) {if(player.getAttribute('src')!==null||!player.paused||image.getAttribute('src')!==null||document.querySelector('.glory-image-viewer'))throw Error('suspended playback/image');report(2);phase=3;}
            else if(phase===3) {const resumed=document.querySelector('audio');if(resumed&&resumed.readyState>=1&&!resumed.seeking&&Math.abs(resumed.currentTime-100)<1){if(!resumed.paused)throw Error('resume autoplay');player=resumed;report(3);phase=4;}}
            else if(phase===4 && !document.querySelector('audio')) {if(player.getAttribute('src')!==null||!player.paused||document.querySelector('.glory-image-viewer'))throw Error('retired playback/image');report(4);clearInterval(timer);}
            if(Date.now()-start>18000)throw Error('timeout');
          }catch(error){console.error(error);reportError();clearInterval(timer);}
          function reportError(){const i=document.querySelector('input');if(i){i.value='-1';i.dispatchEvent(new Event('input',{bubbles:true}));}}
          },30);
        })();"#.into()),..Default::default()
    },move|_| {
        if status.get()<0 || started.elapsed()>Duration::from_secs(20){return Err(ShellError("native media acceptance failed".into()));}
        if status.get()==1 && retired.get()==0 {part.borrow().as_ref().unwrap().suspend()?;retired.set(1);}
        if status.get()==2 && retired.get()==1 {part.borrow().as_ref().unwrap().place_before(None)?;retired.set(2);}
        if status.get()==3 && retired.get()==2 {drop(part.borrow_mut().take());retired.set(3);}
        if status.get()==4 {
            if !wav.offsets.lock().unwrap().iter().any(|v|*v>MAX_RANGE_BYTES as u64){return Err(ShellError("seek never requested a later bounded range".into()));}
            println!("Native WAV metadata, controls, paused state, 100s seek, bounded ranges, paused suspension/resume and disposal; original PNG image, zoom, rotation, fit and disposal passed");return Ok(true);
        }
        Ok(false)
    })?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    desktop::run()?;
    Ok(())
}
