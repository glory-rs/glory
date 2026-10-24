use super::*;
use glory_render::*;

#[derive(Default)]
struct Record(Vec<Op>);
impl Sink for Record {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        self.0.push(op);
        Ok(())
    }
}

#[test]
fn every_field_delta_matches_full_shape_difference_including_resets_and_normalization() {
    let mut bases = vec![
        Props::View,
        Props::Group,
        Props::Text(TextProps {
            content: "old".into(),
            ..Default::default()
        }),
        Props::Text(TextProps {
            content: "old".into(),
            kind: TextKind::Heading(2),
            selectable: false,
        }),
        Props::Button(ButtonProps {
            label: "old".into(),
            ..Default::default()
        }),
        Props::Select(SelectProps {
            entries: std::rc::Rc::new(vec![SelectEntry::Option(SelectOption {
                key: SemanticId::new("one").unwrap(),
                label: "One".into(),
                value: "business".into(),
                disabled: false,
            })]),
            selection: SelectSelection {
                keys: vec![SemanticId::new("one").unwrap()],
                revision: 10,
            },
            ..Default::default()
        }),
        Props::Select(SelectProps {
            multiple: true,
            ..Default::default()
        }),
        Props::Input(InputProps {
            value: "old".into(),
            revision: 10,
            ..Default::default()
        }),
        Props::Input(InputProps {
            value: "old".into(),
            revision: 10,
            multiline: true,
            ..Default::default()
        }),
        Props::Check(CheckProps {
            label: "old".into(),
            ..Default::default()
        }),
        Props::Check(CheckProps {
            mode: CheckMode::Mixed,
            checked: true,
            ..Default::default()
        }),
        Props::Check(CheckProps {
            mode: CheckMode::Switch,
            checked: true,
            ..Default::default()
        }),
        Props::Check(CheckProps {
            mode: CheckMode::Radio,
            checked: true,
            ..Default::default()
        }),
        Props::Image(Default::default()),
        Props::Graphic(GraphicProps::default()),
        Props::Graphic(GraphicProps {
            commands: vec![
                PathCommand::Move(GraphicPoint::new(2., 4.)),
                PathCommand::Line(GraphicPoint::new(22., 4.)),
            ],
            transition: Some(GraphicTransition {
                duration_millis: 200,
            }),
            ..Default::default()
        }),
        Props::Graphic(GraphicProps {
            alternative: Some("Old icon".into()),
            animation: Some(GraphicAnimation::Rotate {
                period_millis: 1000,
            }),
            fill: Some(style::Color(255, 0, 0, 255)),
            stroke: Some(GraphicStroke {
                color: style::Color(0, 0, 255, 128),
                width: 2.0,
                cap: StrokeCap::Round,
            }),
            commands: vec![
                PathCommand::Move(GraphicPoint::new(1.0, 2.0)),
                PathCommand::Line(GraphicPoint::new(20.0, 21.0)),
            ],
            ..Default::default()
        }),
        Props::Image(ImageProps {
            source: "old.png".into(),
            revision: 1,
            alternative: Some("old".into()),
        }),
        Props::Link(LinkProps {
            target: "#old".into(),
            label: "old".into(),
        }),
    ]
    .into_iter()
    .map(Descriptor::new)
    .collect::<Vec<_>>();
    let mut sheet = Descriptor::new(Props::View);
    sheet.role = Some(Role::Dialog);
    sheet.focus = FocusMode::Programmatic;
    sheet.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("oracle-sheet").unwrap()),
        layer: Some(Layer {
            placement: LayerPlacement::Bottom,
            sheet: Some(SheetConfig {
                fractions: vec![0.25, 0.5, 1.0],
                current: 0,
                max_extent: 640.0,
                settle_millis: 180,
                dismissible: true,
                revision: 0,
            }),
            ..Default::default()
        }),
        ..Default::default()
    }));
    bases.push(sheet);
    let mut grip = Descriptor::new(Props::View);
    grip.semantics = Some(Box::new(Semantics {
        sheet_grip: true,
        ..Default::default()
    }));
    bases.push(grip);
    let mut overridden = Descriptor::new(Props::Text(TextProps {
        content: "old".into(),
        ..Default::default()
    }));
    overridden
        .options
        .insert(HtmlTextOptions::default().tag(TextTag::P));
    bases.push(overridden);
    for axis in [Orientation::Horizontal, Orientation::Vertical] {
        let mut separator = Descriptor::new(Props::View);
        separator.role = Some(Role::Separator);
        separator.semantics = Some(Box::new(Semantics {
            separator_axis: Some(axis),
            ..Default::default()
        }));
        bases.push(separator);
    }
    for kind in [TextKind::Code, TextKind::Kbd, TextKind::Mark] {
        bases.push(Descriptor::new(Props::Text(TextProps {
            content: "old".into(),
            kind,
            ..Default::default()
        })));
    }
    for part in [
        DocumentPart::Blockquote,
        DocumentPart::OrderedList,
        DocumentPart::UnorderedList,
        DocumentPart::DescriptionList,
    ] {
        let mut descriptor = Descriptor::new(Props::View);
        descriptor.semantics = Some(Box::new(Semantics {
            document_part: Some(part),
            ..Default::default()
        }));
        bases.push(descriptor);
    }
    let mut named = Descriptor::new(Props::View);
    for current in [None, Some(0.0), Some(25.0), Some(100.0)] {
        let mut progress = Descriptor::new(Props::View);
        progress.role = Some(Role::ProgressBar);
        progress.semantics = Some(Box::new(Semantics {
            progress: Some(ProgressValue {
                min: 0.,
                max: 100.,
                current,
                value_text: Some("old progress".into()),
            }),
            ..Default::default()
        }));
        bases.push(progress);
    }
    named.name = Some("old\0".into());
    bases.push(named);
    let mut inert = Descriptor::new(Props::View);
    inert.semantics = Some(Box::new(Semantics {
        inert: true,
        busy: Some(true),
        ..Default::default()
    }));
    inert.style.pointer_events = Some(style::PointerEvents::None);
    bases.push(inert);
    let mut item = Descriptor::new(Props::View);
    item.role = Some(Role::ListItem);
    item.item = Some(ItemPosition {
        index: 3,
        count: Some(100_000_000),
    });
    bases.push(item);
    let mut fieldset = Descriptor::new(Props::View);
    fieldset.semantics = Some(Box::new(Semantics {
        field_part: Some(FieldPart::Fieldset),
        ..Default::default()
    }));
    bases.push(fieldset);
    let mut patches = vec![
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            field_part: Some(FieldPart::Fieldset),
            invalid: true,
            error_message: Some(SemanticId::new("group-error").unwrap()),
            ..Default::default()
        })))),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            check_group: Some(SemanticId::new("radio-association").unwrap()),
            navigation_item: true,
            ..Default::default()
        })))),
        Patch::Semantics(Change::Keep),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            progress: Some(ProgressValue {
                min: -10.,
                max: 50.,
                current: None,
                value_text: None,
            }),
            ..Default::default()
        })))),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            progress: Some(ProgressValue {
                min: 0.,
                max: 100.,
                current: Some(50.),
                value_text: Some("half\r\n<&>".into()),
            }),
            ..Default::default()
        })))),
        Patch::Semantics(Change::Clear),
        Patch::Semantics(Change::Set(None)),
        Patch::Text(Change::Keep),
        Patch::Text(Change::Set("old".into())),
        Patch::Text(Change::Set("new\r\n\0🙂".into())),
        Patch::Text(Change::Clear),
        Patch::TextKind(Change::Keep),
        Patch::TextKind(Change::Set(TextKind::Paragraph)),
        Patch::TextKind(Change::Set(TextKind::Heading(1))),
        Patch::TextKind(Change::Clear),
        Patch::Selectable(Change::Keep),
        Patch::Selectable(Change::Set(false)),
        Patch::Selectable(Change::Set(true)),
        Patch::Selectable(Change::Clear),
        Patch::Label(Change::Keep),
        Patch::Label(Change::Set("new\r\n\0🙂".into())),
        Patch::Label(Change::Clear),
        Patch::Enabled(Change::Keep),
        Patch::Enabled(Change::Set(false)),
        Patch::Enabled(Change::Set(true)),
        Patch::Enabled(Change::Clear),
        Patch::Value {
            value: Change::Keep,
            revision: u64::MAX,
        },
        Patch::Value {
            value: Change::Set("old".into()),
            revision: 10,
        },
        Patch::Value {
            value: Change::Set("old".into()),
            revision: 11,
        },
        Patch::Value {
            value: Change::Set("new".into()),
            revision: 10,
        },
        Patch::Value {
            value: Change::Set("new\r\n\0🙂".into()),
            revision: 11,
        },
        Patch::Value {
            value: Change::Clear,
            revision: 11,
        },
        Patch::Multiline(Change::Keep),
        Patch::Multiline(Change::Set(false)),
        Patch::Multiline(Change::Set(true)),
        Patch::Multiline(Change::Clear),
        Patch::CheckMode(Change::Keep),
        Patch::CheckMode(Change::Set(CheckMode::Checkbox)),
        Patch::CheckMode(Change::Set(CheckMode::Mixed)),
        Patch::CheckMode(Change::Set(CheckMode::Switch)),
        Patch::CheckMode(Change::Set(CheckMode::Radio)),
        Patch::CheckMode(Change::Clear),
        Patch::SelectSelection {
            keys: Change::Keep,
            revision: 99,
        },
        Patch::SelectSelection {
            keys: Change::Clear,
            revision: 11,
        },
        Patch::SelectSelection {
            keys: Change::Set(vec![SemanticId::new("one").unwrap()]),
            revision: 11,
        },
        Patch::SelectSelection {
            keys: Change::Set(vec![]),
            revision: 10,
        },
        Patch::Checked(Change::Keep),
        Patch::Checked(Change::Set(false)),
        Patch::Checked(Change::Set(true)),
        Patch::Checked(Change::Clear),
        Patch::Source {
            source: Change::Keep,
            revision: 0,
        },
        Patch::Source {
            source: Change::Set("new\0.png".into()),
            revision: 3,
        },
        Patch::Source {
            source: Change::Clear,
            revision: 3,
        },
        Patch::Alternative(Change::Keep),
        Patch::Alternative(Change::Set(Some("new\r\n\0".into()))),
        Patch::Alternative(Change::Set(Some(String::new()))),
        Patch::Alternative(Change::Set(None)),
        Patch::Alternative(Change::Clear),
        Patch::Graphic(Change::Keep),
        Patch::Graphic(Change::Set(Box::new(GraphicProps {
            commands: vec![
                PathCommand::Move(GraphicPoint::new(2., 20.)),
                PathCommand::Line(GraphicPoint::new(22., 20.)),
            ],
            transition: Some(GraphicTransition {
                duration_millis: 300,
            }),
            ..Default::default()
        }))),
        Patch::Graphic(Change::Set(Box::new(GraphicProps {
            commands: vec![
                PathCommand::Move(GraphicPoint::new(2., 20.)),
                PathCommand::Line(GraphicPoint::new(22., 20.)),
            ],
            transition: Some(GraphicTransition {
                duration_millis: 501,
            }),
            ..Default::default()
        }))),
        Patch::Graphic(Change::Clear),
        Patch::Graphic(Change::Set(Box::new(GraphicProps::default()))),
        Patch::Graphic(Change::Set(Box::new(GraphicProps {
            width: 48.0,
            animation: Some(GraphicAnimation::Rotate {
                period_millis: 2000,
            }),
            height: 32.0,
            alternative: Some("New\r\n<&>icon".into()),
            fill: Some(style::Color(1, 2, 3, 4)),
            commands: vec![
                PathCommand::Move(GraphicPoint::new(2.0, 3.0)),
                PathCommand::Cubic {
                    first: GraphicPoint::new(4.0, 5.0),
                    second: GraphicPoint::new(6.0, 7.0),
                    end: GraphicPoint::new(8.0, 9.0),
                },
                PathCommand::Close,
            ],
            ..Default::default()
        }))),
        Patch::Target(Change::Keep),
        Patch::Target(Change::Set("#new\0".into())),
        Patch::Target(Change::Clear),
        Patch::Classes(Change::Keep),
        Patch::Classes(Change::Set(vec!["new\0".into()])),
        Patch::Classes(Change::Set(vec!["old".into()])),
        Patch::Classes(Change::Set(vec!["bad token".into()])),
        Patch::Classes(Change::Clear),
        Patch::Style(Change::Keep),
        Patch::Style(Change::Set(Box::new(
            style::Style::new()
                .display(style::Display::Grid)
                .grid_columns(style::GridTracks::Tracks(vec![
                    style::GridTrack::Px(10.0),
                    style::GridTrack::Percent(20.0),
                    style::GridTrack::Fr(1.0),
                    style::GridTrack::Auto,
                ]))
                .grid_rows(style::GridTracks::Tracks(vec![style::GridTrack::Auto]))
                .grid_column(style::GridPlacement::At { start: 2, span: 3 })
                .grid_row(style::GridPlacement::Span(2))
                .aspect_ratio(style::AspectRatio::Ratio(2.0)),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new()
                .grid_columns(style::GridTracks::None)
                .grid_rows(style::GridTracks::None)
                .grid_column(style::GridPlacement::Auto)
                .grid_row(style::GridPlacement::Auto)
                .aspect_ratio(style::AspectRatio::Auto),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().text_overflow(style::TextOverflow::Clip),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().pointer_events(style::PointerEvents::Auto),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().pointer_events(style::PointerEvents::None),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().object_fit(style::ObjectFit::Fill),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().object_fit(style::ObjectFit::Contain),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().object_fit(style::ObjectFit::Cover),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().object_fit(style::ObjectFit::None),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().object_fit(style::ObjectFit::ScaleDown),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().text_overflow(style::TextOverflow::Ellipsis),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().flex_wrap(style::FlexWrap::Wrap),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().display(style::Display::InlineFlex),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().flex_wrap(style::FlexWrap::NoWrap),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().box_shadow(style::BoxShadow::None),
        ))),
        Patch::Style(Change::Set(Box::new(style::Style::new().box_shadow(
            style::BoxShadow::Outset(style::Shadow {
                x: -2.0,
                y: 3.0,
                blur: 4.0,
                spread: -1.0,
                color: style::Color(10, 20, 30, 40),
            }),
        )))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().field_sizing(style::FieldSizing::Content),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().field_sizing(style::FieldSizing::Fixed),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().width(style::Length::Px(-0.0)),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new()
                .position(style::Position::Fixed)
                .right(style::Length::Px(12.0))
                .bottom(style::Length::Percent(5.0)),
        ))),
        Patch::Style(Change::Set(Box::new(
            style::Style::new()
                .right(style::Length::Auto)
                .bottom(style::Length::Px(-2.0)),
        ))),
        Patch::Style(Change::Set(Box::new(style::Style {
            color: Some(style::Color(1, 2, 3, 255)),
            ..Default::default()
        }))),
        Patch::Style(Change::Clear),
        Patch::Name(Change::Keep),
        Patch::Name(Change::Set(Some("new\r\n\0".into()))),
        Patch::Name(Change::Set(Some(String::new()))),
        Patch::Name(Change::Set(Some("old�".into()))),
        Patch::Name(Change::Set(None)),
        Patch::Name(Change::Clear),
        Patch::Focus(Change::Keep),
        Patch::Focus(Change::Set(FocusMode::Programmatic)),
        Patch::Focus(Change::Set(FocusMode::TabStop)),
        Patch::Focus(Change::Clear),
        Patch::Item(Change::Keep),
        Patch::Item(Change::Set(Some(ItemPosition {
            index: 99_999_999,
            count: None,
        }))),
        Patch::Item(Change::Set(Some(ItemPosition {
            index: usize::MAX,
            count: None,
        }))),
        Patch::Item(Change::Set(None)),
        Patch::Item(Change::Clear),
        Patch::Role(Change::Keep),
        Patch::Role(Change::Set(None)),
        Patch::Role(Change::Clear),
        Patch::Purpose(Change::Keep),
        Patch::Purpose(Change::Clear),
    ];
    for props in [
        Props::Button(ButtonProps::default()),
        Props::Check(CheckProps::default()),
    ] {
        let mut named = Descriptor::new(props);
        named.name = Some("named control".into());
        bases.push(named);
    }
    let semantic_cases = [
        Semantics {
            spinbutton: Some(glory_render::SpinValue {
                min: Some(-10.),
                max: Some(10.),
                value: Some(2.),
                value_text: Some("2.00 exact".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        Semantics {
            spinbutton: Some(glory_render::SpinValue {
                can_increase: false,
                can_decrease: false,
                ..Default::default()
            }),
            readonly: true,
            ..Default::default()
        },
        Semantics {
            slider: Some(SliderValue {
                range: RangeValue {
                    min: -10.,
                    max: 90.,
                    value: 30.,
                },
                axis: Orientation::Vertical,
                direction: Direction::Rtl,
                step: 0.5,
                page_step: 5.,
                value_text: Some("Thirty units".into()),
            }),
            readonly: true,
            required: true,
            form_control: Some(FormControl {
                name: "amount".into(),
                value: Some("30".into()),
            }),
            ..Default::default()
        },
        Semantics {
            splitter: Some(glory_render::RangeValue {
                min: 10.0,
                max: 90.0,
                value: 50.0,
            }),
            separator_axis: Some(Orientation::Vertical),
            controls: Some(SemanticId::new("pane").unwrap()),
            ..Default::default()
        },
        Semantics {
            id: Some(SemanticId::new("decision-layer").unwrap()),
            layer: Some(Layer {
                initial_focus: Some(SemanticId::new("safe-cancel").unwrap()),
                ..Default::default()
            }),
            ..Default::default()
        },
        Semantics {
            busy: Some(true),
            ..Default::default()
        },
        Semantics {
            busy: Some(true),
            inert: true,
            ..Default::default()
        },
        Semantics {
            busy: Some(false),
            inert: false,
            ..Default::default()
        },
        Semantics {
            scroll_keyboard: Some(glory_render::ScrollAxes::Both),
            ..Default::default()
        },
        Semantics {
            scroll_keyboard: Some(glory_render::ScrollAxes::Horizontal),
            ..Default::default()
        },
        Semantics {
            scroll_keyboard: Some(glory_render::ScrollAxes::Vertical),
            ..Default::default()
        },
        Semantics {
            id: Some(SemanticId::new("description-layer").unwrap()),
            layer: Some(Layer {
                modal: false,
                focus: glory_render::LayerFocus::Preserve,
                placement: glory_render::LayerPlacement::Anchor(
                    glory_render::AnchorPlacement::new(
                        glory_render::AnchorSide::Bottom,
                        glory_render::AnchorAlign::Start,
                    ),
                ),
                anchor: Some(SemanticId::new("description-target").unwrap()),
                ..Default::default()
            }),
            ..Default::default()
        },
        Semantics {
            menu_item: Some(glory_render::MenuItemSemantics {
                kind: glory_render::MenuItemKind::Radio,
                owner: SemanticId::new("menu-owner").unwrap(),
            }),
            check_group: Some(SemanticId::new("menu-radios").unwrap()),
            navigation_item: true,
            ..Default::default()
        },
        Semantics {
            id: Some(SemanticId::new("menu-layer").unwrap()),
            navigation: Some(Orientation::Vertical),
            typeahead: true,
            layer: Some(Layer {
                modal: false,
                placement: glory_render::LayerPlacement::Anchor(
                    glory_render::AnchorPlacement::new(
                        glory_render::AnchorSide::Right,
                        glory_render::AnchorAlign::Start,
                    )
                    .gap(4.0),
                ),
                anchor: Some(SemanticId::new("menu-trigger").unwrap()),
                return_focus: Some(SemanticId::new("menu-trigger").unwrap()),
                ..Default::default()
            }),
            ..Default::default()
        },
        Semantics {
            popup: Some(glory_render::PopupKind::Menu),
            ..Default::default()
        },
        Semantics {
            menu_item: Some(glory_render::MenuItemSemantics {
                kind: glory_render::MenuItemKind::Action,
                owner: SemanticId::new("menu-owner").unwrap(),
            }),
            ..Default::default()
        },
        Semantics {
            menu_item: Some(glory_render::MenuItemSemantics {
                kind: glory_render::MenuItemKind::Checkbox,
                owner: SemanticId::new("menu-owner").unwrap(),
            }),
            navigation_item: true,
            ..Default::default()
        },
        Semantics {
            link_request: true,
            ..Default::default()
        },
        Semantics {
            link_open: Some(glory_render::LinkOpen::Current),
            ..Default::default()
        },
        Semantics {
            current: Some(glory_render::Current::Page),
            ..Default::default()
        },
        Semantics {
            current: Some(glory_render::Current::Step),
            ..Default::default()
        },
        Semantics {
            current: Some(glory_render::Current::Location),
            ..Default::default()
        },
        Semantics {
            link_open: Some(glory_render::LinkOpen::NewContext),
            ..Default::default()
        },
        Semantics {
            pressed: Some(false),
            ..Default::default()
        },
        Semantics {
            pressed: Some(true),
            ..Default::default()
        },
        Semantics {
            field_part: Some(FieldPart::Form),
            ..Default::default()
        },
        Semantics {
            field_part: Some(FieldPart::Form),
            labelled_by: Some(SemanticId::new("form-label").unwrap()),
            ..Default::default()
        },
        Semantics {
            form_action: Some(FormAction::Submit),
            form_control: Some(FormControl {
                name: "intent".into(),
                value: Some("save".into()),
            }),
            ..Default::default()
        },
        Semantics {
            form_action: Some(FormAction::Reset),
            ..Default::default()
        },
        Semantics {
            form_control: Some(FormControl {
                name: "value".into(),
                value: None,
            }),
            ..Default::default()
        },
        Semantics {
            required: true,
            readonly: true,
            invalid: true,
            described_by: Some(SemanticId::new("description").unwrap()),
            error_message: Some(SemanticId::new("error").unwrap()),
            ..Default::default()
        },
        Semantics {
            field_part: Some(FieldPart::Label),
            label_for: Some(SemanticId::new("target").unwrap()),
            ..Default::default()
        },
        Semantics {
            field_part: Some(FieldPart::Fieldset),
            disabled: true,
            ..Default::default()
        },
        Semantics {
            id: Some(SemanticId::new("layer").unwrap()),
            described_by: Some(SemanticId::new("description").unwrap()),
            layer: Some(Layer::default()),
            ..Default::default()
        },
        Semantics {
            popup: Some(glory_render::PopupKind::Dialog),
            ..Default::default()
        },
        Semantics::default(),
        Semantics {
            id: Some(SemanticId::new("node").unwrap()),
            labelled_by: Some(SemanticId::new("label").unwrap()),
            hidden: true,
            ..Default::default()
        },
        Semantics {
            expanded: Some(false),
            controls: Some(SemanticId::new("panel").unwrap()),
            navigation_item: true,
            ..Default::default()
        },
        Semantics {
            expanded: Some(true),
            disabled: true,
            ..Default::default()
        },
        Semantics {
            tab: true,
            selected: Some(false),
            controls: Some(SemanticId::new("panel").unwrap()),
            ..Default::default()
        },
        Semantics {
            tab: true,
            selected: Some(true),
            ..Default::default()
        },
        Semantics {
            navigation: Some(Orientation::Horizontal),
            navigation_direction: glory_render::Direction::Rtl,
            typeahead: true,
            ..Default::default()
        },
        Semantics {
            navigation: Some(Orientation::Vertical),
            ..Default::default()
        },
        Semantics {
            id: Some(SemanticId::new("navigation-trigger").unwrap()),
            controls: Some(SemanticId::new("navigation-content").unwrap()),
            expanded: Some(false),
            navigation_item: true,
            navigation_panel: Some(glory_render::NavigationPanel::Trigger),
            ..Default::default()
        },
        Semantics {
            id: Some(SemanticId::new("navigation-content").unwrap()),
            navigation: Some(Orientation::Vertical),
            navigation_panel: Some(glory_render::NavigationPanel::Content),
            ..Default::default()
        },
        Semantics {
            id: Some(SemanticId::new("navigation-content").unwrap()),
            navigation: Some(Orientation::Vertical),
            navigation_panel: Some(glory_render::NavigationPanel::Content),
            layer: Some(glory_render::Layer {
                modal: false,
                focus: glory_render::LayerFocus::Preserve,
                placement: glory_render::LayerPlacement::Anchor(
                    glory_render::AnchorPlacement::new(
                        glory_render::AnchorSide::Bottom,
                        glory_render::AnchorAlign::Start,
                    ),
                ),
                anchor: Some(SemanticId::new("navigation-trigger").unwrap()),
                ..Default::default()
            }),
            ..Default::default()
        },
    ];
    for metadata in semantic_cases {
        patches.push(Patch::Semantics(Change::Set(Some(Box::new(
            metadata.clone(),
        )))));
        for props in [
            Props::Link(LinkProps::default()),
            Props::View,
            Props::Button(ButtonProps::default()),
            Props::Input(InputProps::default()),
            Props::Check(CheckProps::default()),
            Props::Check(CheckProps {
                mode: glory_render::CheckMode::Radio,
                ..Default::default()
            }),
        ] {
            let mut base = Descriptor::new(props);
            if metadata.slider.is_some() && matches!(base.props, Props::View) {
                base.role = Some(Role::Slider);
                base.focus = FocusMode::TabStop;
            }
            if metadata.splitter.is_some() && matches!(base.props, Props::View) {
                base.role = Some(Role::Separator);
                base.focus = FocusMode::TabStop;
            }
            if metadata.layer.is_some() && matches!(base.props, Props::View) {
                let mut decision = base.clone();
                decision.role = Some(Role::AlertDialog);
                decision.focus = FocusMode::Programmatic;
                decision.semantics = Some(Box::new(metadata.clone()));
                if decision.validate().is_ok() {
                    bases.push(decision);
                }
                if metadata
                    .layer
                    .as_ref()
                    .is_some_and(|layer| layer.focus == glory_render::LayerFocus::Preserve)
                {
                    let mut tooltip = base.clone();
                    tooltip.role = Some(Role::Tooltip);
                    tooltip.semantics = Some(Box::new(metadata.clone()));
                    if tooltip.validate().is_ok() {
                        bases.push(tooltip);
                    }
                }
                base.role = Some(if metadata.navigation.is_some() {
                    Role::Menu
                } else {
                    Role::Dialog
                });
                base.focus = FocusMode::Programmatic;
            }
            if metadata.navigation_panel == Some(glory_render::NavigationPanel::Content)
                && matches!(base.props, Props::View)
            {
                base.role = Some(Role::Group);
                base.focus = FocusMode::Auto;
            }
            base.semantics = Some(Box::new(metadata.clone()));
            if metadata.navigation_panel.is_some()
                && (matches!(base.props, Props::Button(_))
                    && metadata.navigation_panel == Some(glory_render::NavigationPanel::Trigger)
                    || matches!(base.props, Props::View)
                        && metadata.navigation_panel
                            == Some(glory_render::NavigationPanel::Content))
            {
                base.validate().unwrap();
            }
            if base.validate().is_ok() {
                bases.push(base);
            }
        }
    }
    for role in [
        Role::TabList,
        Role::TabPanel,
        Role::RadioGroup,
        Role::Toolbar,
        Role::Banner,
        Role::ContentInfo,
        Role::Menu,
        Role::Heading(1),
        Role::Heading(6),
        Role::Heading(0),
        Role::Heading(7),
    ] {
        patches.push(Patch::Role(Change::Set(Some(role))));
        let mut base = Descriptor::new(Props::View);
        base.role = Some(role);
        if base.validate().is_ok() {
            bases.push(base);
        }
    }
    patches.extend(
        [
            Purpose::Text,
            Purpose::Password,
            Purpose::Email,
            Purpose::Url,
            Purpose::Number,
            Purpose::Search,
            Purpose::Tel,
        ]
        .into_iter()
        .map(|purpose| Patch::Purpose(Change::Set(purpose))),
    );
    patches.extend(
        [
            Role::Group,
            Role::Main,
            Role::Navigation,
            Role::Section,
            Role::List,
            Role::ListItem,
            Role::Status,
            Role::Alert,
            Role::Table,
            Role::Row,
            Role::ColumnHeader,
            Role::Cell,
            Role::Dialog,
            Role::AlertDialog,
            Role::Tooltip,
        ]
        .into_iter()
        .map(|role| Patch::Role(Change::Set(Some(role)))),
    );
    for (revision, current) in [(0, 0), (0, 1), (1, 2)] {
        patches.push(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            id: Some(SemanticId::new("oracle-sheet").unwrap()),
            layer: Some(Layer {
                placement: LayerPlacement::Bottom,
                sheet: Some(SheetConfig {
                    fractions: vec![0.25, 0.5, 1.0],
                    current,
                    max_extent: 640.0,
                    settle_millis: 180,
                    dismissible: true,
                    revision,
                }),
                ..Default::default()
            }),
            ..Default::default()
        })))));
    }
    patches.push(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
        sheet_grip: true,
        ..Default::default()
    })))));
    let (mut accepted, mut rejected) = (0, 0);
    for limit in [
        style::Limit::None,
        style::Limit::Px(25.0),
        style::Limit::Percent(50.0),
        style::Limit::Px(-1.0),
        style::Limit::Px(f64::NAN),
        style::Limit::Percent(f64::INFINITY),
    ] {
        patches.push(Patch::Style(Change::Set(Box::new(
            style::Style::new().max_width(limit).max_height(limit),
        ))));
    }
    for mut base in bases {
        if !matches!(base.props, Props::Group) {
            base.classes = vec!["old".into()];
            base.style = style::Style {
                width: Some(style::Length::Px(0.0)),
                background: Some(style::Color(8, 9, 10, 255)),
                display: Some(style::Display::Grid),
                grid_columns: Some(style::GridTracks::Tracks(vec![
                    style::GridTrack::Fr(1.0),
                    style::GridTrack::Auto,
                ])),
                grid_rows: Some(style::GridTracks::Tracks(vec![style::GridTrack::Px(20.0)])),
                grid_column: Some(style::GridPlacement::At { start: 1, span: 2 }),
                grid_row: Some(style::GridPlacement::Span(1)),
                aspect_ratio: Some(style::AspectRatio::Ratio(2.0)),
                object_fit: Some(style::ObjectFit::Cover),
                pointer_events: Some(style::PointerEvents::None),
                box_shadow: Some(style::BoxShadow::None),
                flex_wrap: Some(style::FlexWrap::Wrap),
                text_overflow: Some(style::TextOverflow::Ellipsis),
                ..Default::default()
            };
        }
        for update in &patches {
            let expected = base.patched(update.clone()).and_then(|next| {
                difference(
                    &shape(&base)?,
                    &shape(&next)?,
                    matches!(update, Patch::Checked(Change::Set(_) | Change::Clear)),
                    matches!(update, Patch::CheckMode(Change::Set(_) | Change::Clear)),
                    matches!(
                        update,
                        Patch::SelectSelection {
                            keys: Change::Set(_) | Change::Clear,
                            ..
                        }
                    ),
                )
            });
            for planned in [false, true] {
                let runtime = RuntimeId::new();
                let id = NodeIds::new(runtime).allocate();
                let mut mapper = Mapper::new(runtime);
                let mut sink = Record::default();
                mapper
                    .apply(
                        &[
                            if planned {
                                Mutation::CreatePlan {
                                    plan: Plan::new(vec![PlanNode {
                                        parent: None,
                                        descriptor: base.clone(),
                                    }])
                                    .unwrap(),
                                    ids: vec![id],
                                }
                            } else {
                                Mutation::Create {
                                    id,
                                    node: Box::new(base.clone()),
                                }
                            },
                            Mutation::Place {
                                id,
                                parent: None,
                                before: None,
                            },
                        ],
                        &mut sink,
                    )
                    .unwrap();
                let receipt = mapper.receipt();
                let stats = mapper.tree().stats();
                let ops = sink.0.len();
                let actual = mapper.apply(
                    &[Mutation::Patch {
                        id,
                        patch: update.clone(),
                    }],
                    &mut sink,
                );
                match &expected {
                    Ok(expected) => {
                        accepted += 1;
                        assert_eq!(actual.unwrap().applied, receipt.applied + 1);
                        let Some(Op::Patch { edits, .. }) = sink.0.last() else {
                            panic!()
                        };
                        assert_eq!(edits, expected, "{:?}/{update:?}", base.props);
                    }
                    Err(_) => {
                        rejected += 1;
                        assert!(actual.is_err(), "{:?}/{update:?}", base.props);
                        assert_eq!(mapper.tree().node(id).unwrap().descriptor, base);
                        assert_eq!(mapper.tree().stats(), stats);
                        assert_eq!(mapper.receipt(), receipt);
                        assert_eq!(sink.0.len(), ops);
                    }
                }
            }
        }
    }
    assert!(accepted > 300 && rejected > 300);
    println!("field delta oracle: accepted={accepted}, rejected={rejected}");
}

#[test]
fn mapper_metadata_keeps_large_unrelated_payloads_and_emits_no_value_property() {
    for props in [
        Props::Input(InputProps {
            value: "x".repeat(1_048_576),
            ..Default::default()
        }),
        Props::Text(TextProps {
            content: "x".repeat(1_048_576),
            kind: TextKind::Heading(2),
            ..Default::default()
        }),
    ] {
        let runtime = RuntimeId::new();
        let id = NodeIds::new(runtime).allocate();
        let mut mapper = Mapper::new(runtime);
        let mut sink = Record::default();
        mapper
            .apply(
                &[
                    Mutation::Create {
                        id,
                        node: Box::new(Descriptor::new(props)),
                    },
                    Mutation::Place {
                        id,
                        parent: None,
                        before: None,
                    },
                ],
                &mut sink,
            )
            .unwrap();
        let pointer =
            |mapper: &Mapper| match &mapper.tree().node(id).unwrap().descriptor.view().props {
                Props::Input(input) => input.value.as_ptr(),
                Props::Text(text) => text.content.as_ptr(),
                _ => unreachable!(),
            };
        let original = pointer(&mapper);
        for patch in [
            Patch::Name(Change::Set(Some("Name".into()))),
            Patch::Name(Change::Keep),
            Patch::Name(Change::Clear),
            Patch::Focus(Change::Set(FocusMode::Programmatic)),
        ] {
            mapper
                .apply(&[Mutation::Patch { id, patch }], &mut sink)
                .unwrap();
            assert_eq!(pointer(&mapper), original);
            let Some(Op::Patch { edits, .. }) = sink.0.last() else {
                panic!()
            };
            assert!(
                edits
                    .iter()
                    .all(|edit| matches!(edit, Edit::Attribute { .. }))
            );
        }
    }
}
