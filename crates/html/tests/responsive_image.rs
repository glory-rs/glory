use glory_html::{Kind, Part, shape};
use glory_render::*;

fn attributes(loading: ImageLoading) -> std::collections::BTreeMap<&'static str, String> {
    let descriptor = Descriptor::new(Props::Image(ImageProps {
        source: "default.png".into(),
        alternative: Some("Responsive sample".into()),
        responsive: ResponsiveImage {
            candidates: vec![
                ImageCandidate {
                    source: "small.png".into(),
                    width: 320,
                },
                ImageCandidate {
                    source: "large.png".into(),
                    width: 960,
                },
            ],
            sizes: vec![
                ImageSizeHint {
                    max_viewport_width: Some(600),
                    slot: ImageSlotSize::ViewportWidth(100),
                },
                ImageSizeHint {
                    max_viewport_width: None,
                    slot: ImageSlotSize::Pixels(480),
                },
            ],
        },
        loading,
        ..Default::default()
    }));
    let mapped = shape(&descriptor).unwrap();
    let Kind::Element { attributes, .. } = &mapped
        .pieces
        .iter()
        .find(|piece| piece.part == Part::Main)
        .unwrap()
        .kind
    else {
        panic!()
    };
    attributes.clone()
}

#[test]
fn responsive_candidates_sizes_and_loading_are_real_img_attributes() {
    let attrs = attributes(ImageLoading::Lazy);
    assert_eq!(attrs.get("src").unwrap(), "default.png");
    assert_eq!(
        attrs.get("srcset").unwrap(),
        "small.png 320w, large.png 960w"
    );
    assert_eq!(
        attrs.get("sizes").unwrap(),
        "(max-width: 600px) 100vw, 480px"
    );
    assert_eq!(attrs.get("loading").unwrap(), "lazy");
    assert!(!attributes(ImageLoading::Auto).contains_key("loading"));
}
