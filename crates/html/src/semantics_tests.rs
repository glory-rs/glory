use crate::{Kind, Part, shape};
use glory_render::*;
use std::collections::BTreeMap;

fn attributes(node: &Descriptor, part: Part) -> BTreeMap<&'static str, String> {
    let shape = shape(node).unwrap();
    let piece = shape
        .pieces
        .iter()
        .find(|piece| piece.part == part)
        .unwrap();
    let Kind::Element { attributes, .. } = &piece.kind else {
        panic!("expected an element")
    };
    attributes.clone()
}

#[test]
fn range_controls_keep_numeric_coordinates_and_exact_text_separate() {
    let mut progress = Descriptor::new(Props::View);
    progress.role = Some(Role::ProgressBar);
    progress.semantics = Some(Box::new(Semantics {
        progress: Some(ProgressValue {
            min: -2.5,
            max: 8.75,
            current: Some(1.25),
            value_text: Some("1.2500 exact\r\nkg".into()),
        }),
        ..Default::default()
    }));
    let mut spin = Descriptor::new(Props::Input(InputProps::default()));
    spin.semantics = Some(Box::new(Semantics {
        spinbutton: Some(SpinValue {
            min: Some(-2.5),
            max: Some(8.75),
            value: Some(1.25),
            value_text: Some("1.2500 exact\r\nkg".into()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    let mut slider = Descriptor::new(Props::View);
    slider.role = Some(Role::Slider);
    slider.focus = FocusMode::TabStop;
    slider.semantics = Some(Box::new(Semantics {
        slider: Some(SliderValue {
            range: RangeValue {
                min: -2.5,
                max: 8.75,
                value: 1.25,
            },
            axis: Orientation::Horizontal,
            direction: Direction::Rtl,
            step: 0.25,
            page_step: 1.0,
            value_text: Some("1.2500 exact\r\nkg".into()),
        }),
        ..Default::default()
    }));
    let mut splitter = Descriptor::new(Props::View);
    splitter.role = Some(Role::Separator);
    splitter.focus = FocusMode::TabStop;
    splitter.semantics = Some(Box::new(Semantics {
        splitter: Some(RangeValue {
            min: -2.5,
            max: 8.75,
            value: 1.25,
        }),
        separator_axis: Some(Orientation::Vertical),
        controls: Some(SemanticId::new("pane").unwrap()),
        ..Default::default()
    }));
    for (node, role, text) in [
        (progress, "progressbar", Some("1.2500 exact\nkg")),
        (spin, "spinbutton", Some("1.2500 exact\nkg")),
        (slider, "slider", Some("1.2500 exact\nkg")),
        (splitter, "separator", None),
    ] {
        let attrs = attributes(&node, Part::Main);
        assert_eq!(attrs["role"], role);
        assert_eq!(attrs["aria-valuemin"], "-2.5");
        assert_eq!(attrs["aria-valuemax"], "8.75");
        assert_eq!(attrs["aria-valuenow"], "1.25");
        assert_eq!(attrs.get("aria-valuetext").map(String::as_str), text);
    }
}

#[test]
fn indeterminate_progress_and_unbounded_spin_do_not_invent_coordinates() {
    let mut progress = Descriptor::new(Props::View);
    progress.role = Some(Role::ProgressBar);
    progress.semantics = Some(Box::new(Semantics {
        progress: Some(ProgressValue {
            min: 0.0,
            max: 100.0,
            current: None,
            value_text: None,
        }),
        ..Default::default()
    }));
    let attrs = attributes(&progress, Part::Main);
    assert_eq!(attrs["aria-valuemin"], "0");
    assert_eq!(attrs["aria-valuemax"], "100");
    assert!(!attrs.contains_key("aria-valuenow"));
    assert!(!attrs.contains_key("aria-valuetext"));
    let mut spin = Descriptor::new(Props::Input(InputProps::default()));
    spin.semantics = Some(Box::new(Semantics {
        spinbutton: Some(SpinValue::default()),
        ..Default::default()
    }));
    let attrs = attributes(&spin, Part::Main);
    assert_eq!(attrs["role"], "spinbutton");
    for key in [
        "aria-valuemin",
        "aria-valuemax",
        "aria-valuenow",
        "aria-valuetext",
    ] {
        assert!(!attrs.contains_key(key));
    }
}

#[test]
fn structural_roles_and_intrinsic_control_roles_keep_their_physical_target() {
    let mut heading = Descriptor::new(Props::View);
    heading.role = Some(Role::Heading(3));
    let attrs = attributes(&heading, Part::Main);
    assert_eq!(attrs["role"], "heading");
    assert_eq!(attrs["aria-level"], "3");
    let cleared = heading.patched(Patch::Role(Change::Clear)).unwrap();
    let attrs = attributes(&cleared, Part::Main);
    assert!(!attrs.contains_key("role"));
    assert!(!attrs.contains_key("aria-level"));
    let mut named = Descriptor::new(Props::View);
    named.name = Some("Named group".into());
    assert_eq!(attributes(&named, Part::Main)["role"], "group");
    let switch = Descriptor::new(Props::Check(CheckProps {
        mode: CheckMode::Switch,
        ..Default::default()
    }));
    assert_eq!(attributes(&switch, Part::Control)["role"], "switch");
    assert!(!attributes(&switch, Part::Main).contains_key("role"));
}
