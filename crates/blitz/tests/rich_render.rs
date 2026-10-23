#![cfg(all(not(target_arch = "wasm32"), feature = "raster"))]
use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
use glory_blitz::{Config, Raster, Renderer};
use glory_render::rich_text::{Alignment, Block, Document, Inline, Kind, Marks};
use glory_ui::{App, rich_text};
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct ImageProvider(Mutex<Option<(String, Box<dyn NetHandler>)>>);
impl NetProvider for ImageProvider {
    fn fetch(&self, _document: usize, request: Request, handler: Box<dyn NetHandler>) {
        assert_eq!(request.url.as_str(), "asset://app/image.png");
        assert!(
            self.0
                .lock()
                .unwrap()
                .replace((request.url.to_string(), handler))
                .is_none()
        );
    }
}
fn leaf(id: u64, kind: Kind, text: &str) -> Block {
    Block {
        id,
        kind,
        spans: vec![Inline::Text {
            text: text.into(),
            marks: Marks::default(),
        }],
        children: vec![],
    }
}
#[test]
fn native_rich_document_paints_actual_image_and_styled_table_list() {
    let provider = Arc::new(ImageProvider::default());
    let mut renderer = None;
    let app = App::new(|id| {
        let r = Renderer::with_config(
            id,
            Config {
                provider: Some(provider.clone()),
                ..Config::default()
            },
        );
        renderer = Some(r.clone());
        r
    });
    let renderer = renderer.unwrap();
    renderer.set_viewport(600, 500, 1.0).unwrap();
    renderer.set_css("html,body{background:white;color:black;min-height:500px}");
    let mut paragraph = leaf(2, Kind::Paragraph, "Styled text 中文 ");
    if let Inline::Text { marks, .. } = &mut paragraph.spans[0] {
        *marks = Marks {
            bold: true,
            italic: true,
            strike: true,
            ..Marks::default()
        };
    }
    paragraph.spans.push(Inline::Image {
        source: "asset://app/image.png".into(),
        alternative: "Orange sample".into(),
        title: String::new(),
    });
    paragraph.spans.push(Inline::Text {
        text: " after image".into(),
        marks: Marks::default(),
    });
    let table = Block {
        id: 3,
        kind: Kind::Table(vec![Alignment::Left, Alignment::Right]),
        spans: vec![],
        children: vec![
            Block {
                id: 4,
                kind: Kind::TableHead,
                spans: vec![],
                children: vec![leaf(5, Kind::Cell, "Name"), leaf(6, Kind::Cell, "Count")],
            },
            Block {
                id: 7,
                kind: Kind::Row,
                spans: vec![],
                children: vec![
                    leaf(8, Kind::Cell, "Native image"),
                    leaf(9, Kind::Cell, "1"),
                ],
            },
        ],
    };
    let list = Block {
        id: 10,
        kind: Kind::List(None),
        spans: vec![],
        children: vec![Block {
            id: 11,
            kind: Kind::Item(Some(true)),
            spans: vec![],
            children: vec![leaf(12, Kind::Paragraph, "Completed task")],
        }],
    };
    app.mount(rich_text().document(Document {
        root: Block {
            id: 0,
            kind: Kind::Root,
            spans: vec![],
            children: vec![
                leaf(1, Kind::Heading(1), "Rich text / 原生排版"),
                paragraph,
                table,
                list,
            ],
        },
    }))
    .unwrap();
    app.settle(8).unwrap();
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    let (url, handler) = provider
        .0
        .lock()
        .unwrap()
        .take()
        .expect("image requested from explicit provider");
    let mut encoded = vec![];
    {
        let mut encoder = png::Encoder::new(&mut encoded, 8, 8);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[245, 110, 20, 255].repeat(64))
            .unwrap();
    }
    handler.bytes(url, Bytes::from(encoded));
    renderer.poll_resources();
    raster.draw(&renderer).unwrap();
    let orange = raster
        .pixels()
        .chunks_exact(4)
        .filter(|p| p[0] > 220 && p[1] > 70 && p[1] < 150 && p[2] < 50 && p[3] > 200)
        .count();
    assert!(orange > 5000, "actual image pixels missing: {orange}");
    if let Ok(directory) = std::env::var("GLORY_TEST_ARTIFACT_DIR") {
        let path = std::path::Path::new(&directory);
        assert!(path.is_absolute());
        std::fs::create_dir_all(path).unwrap();
        let file = std::fs::File::create(path.join("native-rich-document.png")).unwrap();
        let mut encoder = png::Encoder::new(file, 600, 500);
        encoder.set_color(png::ColorType::Rgba);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(raster.pixels())
            .unwrap();
    }
}
