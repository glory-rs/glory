#![cfg(feature = "css")]
use glory_render::style::Sheet;
use glory_ui::{check, group, input, view};

#[test]
fn document_embeds_ordered_checked_sheets_with_nonce_and_shared_semantic_states() {
    let document = glory_ssr::Document {
        sheets: vec![
            Sheet::parse(".box {width:10px}").unwrap(),
            Sheet::parse(".box {width:20px}").unwrap(),
        ],
        nonce: Some("nonce\"&".into()),
        ..Default::default()
    };
    let html = glory_ssr::render_document(
        group().fill((
            view().class("box"),
            check().checked(true).enabled(false),
            input(),
        )),
        &document,
    )
    .unwrap();
    assert_eq!(
        html.matches("data-gly-sheet=\"v1\" nonce=\"nonce&quot;&amp;\"")
            .count(),
        2
    );
    assert!(html.find("width:10px").unwrap() < html.find("width:20px").unwrap());
    assert!(html.contains("data-gly-checked=\"\""));
    assert!(html.contains("data-gly-disabled=\"\""));
    assert!(html.contains("data-gly-enabled=\"\""));
}
