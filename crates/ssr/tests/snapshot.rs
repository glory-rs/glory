use glory_render::{
    Change, Descriptor, Error, InputProps, Mutation, NodeIds, Patch, Props, Render, RuntimeId,
    TextKind, TextProps,
};
use glory_ui::{button, check, group, image, input, list, text, view};

#[test]
fn input_purposes_keep_literal_strings_and_multiline_output_has_one_shared_canonical_form() {
    let output = glory_ssr::render(
        group().fill((
            input()
                .purpose(glory_render::Purpose::Email)
                .value("  a\0🙂b  "),
            input()
                .purpose(glory_render::Purpose::Url)
                .value("  https://example.test/a  "),
            input().multiline(true).value("\r\nfirst\rsecond\0🙂"),
        )),
    )
    .unwrap();
    assert!(output.contains("inputmode=\"email\""));
    assert!(output.contains("inputmode=\"url\""));
    assert_eq!(output.matches("type=\"text\"").count(), 2);
    assert!(output.contains("value=\"  a�🙂b  \""));
    assert!(output.contains("value=\"  https://example.test/a  \""));
    assert!(output.contains(">\n\nfirst\nsecond�🙂</textarea>"));
}

#[test]
fn virtual_item_positions_use_logical_totals_and_focus_policy() {
    let output = glory_ssr::render(
        list(
            100_000_000_usize,
            |index| index,
            |_, key| text().content(key.to_string()),
        )
        .height(240.0)
        .overscan(2)
        .name("Items & more")
        .position(2_399_999_760.0),
    )
    .unwrap();
    assert!(output.contains("aria-label=\"Items &amp; more\""));
    assert!(output.contains("data-gly-virtual=\"root\""));
    assert!(output.contains("tabindex=\"0\""));
    assert!(output.contains("tabindex=\"-1\""));
    assert!(output.contains("aria-posinset=\"100000000\""));
    assert_eq!(output.matches("aria-setsize=\"100000000\"").count(), 12);
    let unknown = glory_ssr::render(view().role(glory_ui::Role::ListItem).item(Some(
        glory_ui::ItemPosition {
            index: 12,
            count: None,
        },
    )))
    .unwrap();
    assert!(unknown.contains("aria-posinset=\"13\""));
    assert!(unknown.contains("aria-setsize=\"-1\""));
    assert!(!unknown.contains("tabindex"));
}

#[test]
fn typed_background_color_does_not_emit_a_resetting_shorthand() {
    let style = glory_render::style::Style {
        background: Some(glory_render::style::Color(1, 2, 3, 255)),
        ..Default::default()
    };
    let html = glory_ssr::render(view().class("background").style(style)).unwrap();
    assert!(html.contains("background-color:#010203ff;"));
    assert!(!html.contains("background:"));
}

#[test]
fn snapshot_escapes_values_preserves_children_and_is_runtime_independent() {
    fn output() -> String {
        glory_ssr::render(group().fill((
            button().label("<&\"").fill(text().content("icon")),
            input().value("<&\""),
            input().multiline(true).value("\nline<&"),
            check().label("label").checked(false),
            image().source("a\"b.png").alt("<&"),
        )))
        .unwrap()
    }
    let html = output();
    assert_eq!(html, output());
    assert!(html.starts_with("<!--gly:s:0-->"));
    assert!(html.ends_with("<!--gly:e:0-->"));
    assert!(html.contains("&lt;&amp;&quot;"));
    assert!(html.contains("icon</span></button>"));
    assert!(html.contains(">\n\nline&lt;&amp;</textarea>"));
    assert!(!html.contains(" checked="));
    assert!(!html.contains("data-gly-node"));
}

#[test]
fn canonical_snapshot_keys_ignore_completion_order_and_hidden_history() {
    fn output(reverse: bool, hidden: bool) -> String {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let root = ids.allocate();
        let a = ids.allocate();
        let b = ids.allocate();
        let ghost = ids.allocate();
        let mut renderer = glory_ssr::Renderer::new(runtime);
        let text = |value: &str| {
            Box::new(Descriptor::new(Props::Text(TextProps {
                content: value.into(),
                ..Default::default()
            })))
        };
        let mut mutations = vec![
            Mutation::Create {
                id: root,
                node: Box::new(Descriptor::new(Props::Group)),
            },
            Mutation::Place {
                id: root,
                parent: None,
                before: None,
            },
        ];
        for (id, label) in if reverse {
            [(b, "b"), (a, "a")]
        } else {
            [(a, "a"), (b, "b")]
        } {
            mutations.push(Mutation::Create {
                id,
                node: text(label),
            });
        }
        if hidden {
            mutations.extend([
                Mutation::Create {
                    id: ghost,
                    node: text("hidden"),
                },
                Mutation::Place {
                    id: ghost,
                    parent: Some(root),
                    before: None,
                },
                Mutation::Suspend { id: ghost },
            ]);
        }
        mutations.extend([
            Mutation::Place {
                id: a,
                parent: Some(root),
                before: None,
            },
            Mutation::Place {
                id: b,
                parent: Some(root),
                before: None,
            },
        ]);
        renderer.apply(&mutations).unwrap();
        renderer.snapshot().unwrap()
    }
    assert_eq!(output(false, false), output(true, false));
    assert_eq!(output(false, false), output(true, true));
}

#[test]
fn versioned_snapshots_have_text_boundaries_normalized_strings_and_flow_preview() {
    let snapshot = glory_ssr::snapshot_with(|scope| {
        let empty = scope.cage(String::new());
        group().fill((
            text().content(empty),
            text().content("a\r\nb\0"),
            list(
                100_000_000_usize,
                |index| index,
                |_, key| text().content(format!("row {key}")),
            )
            .measure(24.0)
            .position(24_000_000.0),
        ))
    })
    .unwrap();
    assert!(
        snapshot
            .html
            .starts_with(&format!("<!--gly:v:{}-->", glory_html::FORMAT_VERSION))
    );
    assert!(
        snapshot
            .html
            .ends_with(&format!("<!--gly:/v:{}-->", glory_html::FORMAT_VERSION))
    );
    assert!(snapshot.html.contains("<!--gly:t:1--></span>"));
    assert!(snapshot.html.contains("a\nb\u{fffd}"));
    assert!(snapshot.html.contains("data-gly-virtual=\"surface\""));
    assert!(snapshot.html.contains("data-gly-virtual=\"row\""));
    assert!(!snapshot.html.contains("data-gly-live"));
    assert!(!snapshot.html.contains("data-gly-node"));
    assert!(snapshot.html.contains("row 1000000"));
    assert!(snapshot.html.len() < 20_000);
}

#[test]
fn updates_clear_false_empty_and_remove_only_the_intrinsic_text_slot() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let button = ids.allocate();
    let label = ids.allocate();
    let check = ids.allocate();
    let input = ids.allocate();
    let mut renderer = glory_ssr::Renderer::new(runtime);
    renderer
        .apply(&[
            Mutation::Create {
                id: button,
                node: Box::new(Descriptor::new(Props::Button(Default::default()))),
            },
            Mutation::Place {
                id: button,
                parent: None,
                before: None,
            },
            Mutation::Create {
                id: label,
                node: Box::new(Descriptor::new(Props::Text(TextProps {
                    content: "child".into(),
                    ..Default::default()
                }))),
            },
            Mutation::Place {
                id: label,
                parent: Some(button),
                before: None,
            },
            Mutation::Create {
                id: check,
                node: Box::new(Descriptor::new(Props::Check(glory_render::CheckProps {
                    checked: true,
                    ..Default::default()
                }))),
            },
            Mutation::Place {
                id: check,
                parent: None,
                before: None,
            },
            Mutation::Create {
                id: input,
                node: Box::new(Descriptor::new(Props::Input(InputProps {
                    value: "original".into(),
                    ..Default::default()
                }))),
            },
            Mutation::Place {
                id: input,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    renderer
        .apply(&[
            Mutation::Patch {
                id: button,
                patch: Patch::Label(Change::Set("new".into())),
            },
            Mutation::Patch {
                id: check,
                patch: Patch::Checked(Change::Set(false)),
            },
            Mutation::Patch {
                id: input,
                patch: Patch::Value {
                    value: Change::Set(String::new()),
                    revision: 3,
                },
            },
        ])
        .unwrap();
    let html = renderer.html().unwrap();
    assert!(html.contains(">new<span"));
    assert!(html.contains("child</span></button>"));
    assert!(!html.contains(" checked="));
    assert!(html.contains("value=\"\""));
    assert!(!html.contains("original"));
}

#[test]
fn mapper_rejects_conflicting_tags_and_live_tag_changes_before_applying() {
    let mut descriptor = Descriptor::new(Props::Text(TextProps {
        kind: TextKind::Heading(2),
        ..Default::default()
    }));
    descriptor
        .options
        .insert(glory_html::HtmlTextOptions::default().tag(glory_html::TextTag::P));
    assert!(matches!(
        glory_html::shape(&descriptor),
        Err(Error::InvalidProps(_))
    ));
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut renderer = glory_ssr::Renderer::new(runtime);
    renderer
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::Text(Default::default()))),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let original = renderer.html().unwrap();
    assert!(matches!(
        renderer.apply(&[Mutation::Patch {
            id,
            patch: Patch::TextKind(Change::Set(TextKind::Paragraph))
        }]),
        Err(Error::InvalidProps(_))
    ));
    assert_eq!(renderer.html().unwrap(), original);
}

#[test]
fn server_list_builds_only_a_window_and_document_metadata_is_escaped() {
    let document = glory_ssr::Document {
        title: "<&".into(),
        lang: Some("en\"".into()),
        stylesheets: vec![
            "/style.css?a=1&b=2".into(),
            "/style.css?a=1&b=2".into(),
            "/other.css".into(),
        ],
        nonce: Some("n\"&".into()),
        #[cfg(feature = "css")]
        sheets: Vec::new(),
    };
    let html = glory_ssr::render_document(
        view().fill(list(
            100_000_000_usize,
            |index| index,
            |_, key| text().content(format!("row {key}")),
        )),
        &document,
    )
    .unwrap();
    assert!(html.starts_with("<!doctype html><html lang=\"en&quot;\""));
    assert!(html.contains("<title>&lt;&amp;</title>"));
    assert!(html.contains("/style.css?a=1&amp;b=2"));
    assert_eq!(html.matches("/style.css?a=1&amp;b=2").count(), 1);
    assert_eq!(html.matches("<link ").count(), 2);
    assert!(html.contains("data-gly-base=\"v1\" nonce=\"n&quot;&amp;\""));
    assert_eq!(html.matches("role=\"listitem\"").count(), 29);
    assert!(html.len() < 20_000);
}

#[test]
fn empty_commits_preserve_the_query_watermark_and_snapshots_skip_suspended_anchors() {
    use glory_render::{Input, Query, QueryToken};
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let a = ids.allocate();
    let b = ids.allocate();
    let c = ids.allocate();
    let x = ids.allocate();
    let mut renderer = glory_ssr::Renderer::new(runtime);
    let mut commands = Vec::new();
    for (id, content) in [(a, "aaa"), (b, "bbb"), (c, "ccc")] {
        commands.push(Mutation::Create {
            id,
            node: Box::new(Descriptor::new(Props::Text(TextProps {
                content: content.into(),
                ..Default::default()
            }))),
        });
        commands.push(Mutation::Place {
            id,
            parent: None,
            before: None,
        });
    }
    commands.push(Mutation::Suspend { id: b });
    commands.push(Mutation::Create {
        id: x,
        node: Box::new(Descriptor::new(Props::Text(TextProps {
            content: "xxx".into(),
            ..Default::default()
        }))),
    });
    commands.push(Mutation::Place {
        id: x,
        parent: None,
        before: Some(b),
    });
    let receipt = renderer.apply(&commands).unwrap();
    assert_eq!(renderer.apply(&[]).unwrap(), receipt);
    renderer.request(Query::Bounds(x), QueryToken(1)).unwrap();
    let mut replies = Vec::new();
    renderer.drain(&mut replies);
    assert!(
        matches!(replies.as_slice(), [Input::Response { applied, result: Err(Error::Unsupported(_)), .. }] if *applied == receipt.applied)
    );
    let html = renderer.html().unwrap();
    assert!(!html.contains("bbb"));
    assert!(html.find("aaa").unwrap() < html.find("xxx").unwrap());
    assert!(html.find("xxx").unwrap() < html.find("ccc").unwrap());
    renderer
        .apply(&[Mutation::Resume {
            id: b,
            parent: None,
            before: Some(c),
        }])
        .unwrap();
    let html = renderer.html().unwrap();
    assert!(html.find("xxx").unwrap() < html.find("bbb").unwrap());
    assert!(html.find("bbb").unwrap() < html.find("ccc").unwrap());
}
