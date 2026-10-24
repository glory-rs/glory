use glory_render::*;
fn point(block: usize, offset: usize) -> RichPoint {
    RichPoint { block, offset }
}
fn selection(start: (usize, usize), end: (usize, usize)) -> RichSelection {
    RichSelection {
        anchor: point(start.0, start.1),
        focus: point(end.0, end.1),
    }
}
fn document(text: &str) -> RichDocument {
    RichDocument::new(
        text.split('\n')
            .map(|text| RichBlock {
                kind: RichBlockKind::Paragraph,
                runs: vec![RichRun {
                    text: text.into(),
                    marks: RichMarks::default(),
                }],
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn finite_canonical_model_preserves_plain_text_and_merges_equal_marks() {
    let doc = RichDocument::new(vec![RichBlock {
        kind: RichBlockKind::Heading(2),
        runs: vec![
            RichRun {
                text: "Hello ".into(),
                marks: Default::default(),
            },
            RichRun {
                text: "world 😀".into(),
                marks: Default::default(),
            },
        ],
    }])
    .unwrap();
    assert_eq!(doc.blocks[0].runs.len(), 1);
    assert_eq!(doc.plain_text(), "Hello world 😀");
    assert_eq!(doc.blocks[0].utf16_len(), 14);
    assert!(doc.charged_bytes() > doc.plain_text().len());
    assert_eq!(RichDocument::default().plain_text(), "");
    let quote = doc
        .format(
            selection((0, 0), (0, 5)),
            RichFormat::Block(RichBlockKind::Quote),
        )
        .unwrap();
    assert_eq!(quote.blocks[0].kind, RichBlockKind::Quote);
    assert!(
        doc.format(
            selection((0, 0), (0, 5)),
            RichFormat::Block(RichBlockKind::Heading(7))
        )
        .is_err()
    );
}
#[test]
fn utf16_boundaries_and_backward_ranges_are_exact() {
    let doc = document("A😀B\nSecond");
    assert!(doc.validate_selection(selection((0, 2), (1, 2))).is_err());
    assert!(doc.validate_selection(selection((2, 0), (2, 0))).is_err());
    assert!(doc.validate_selection(selection((0, 5), (0, 5))).is_err());
    let (next, caret) = doc
        .replace(selection((1, 3), (0, 1)), "X\r\nY\0", RichMarks::default())
        .unwrap();
    assert_eq!(next.plain_text(), "AX\nY�ond");
    assert_eq!(caret, RichSelection::collapsed(point(1, 2)));
    assert_eq!(
        selection((1, 1), (0, 0)).ordered(),
        (point(0, 0), point(1, 1))
    );
}
#[test]
fn formatting_splits_selected_utf16_runs_and_canonical_clear_rejoins() {
    let doc = document("One 😀 two");
    let selected = selection((0, 4), (0, 6));
    let bold = doc.format(selected, RichFormat::Bold(true)).unwrap();
    assert_eq!(bold.blocks[0].runs.len(), 3);
    assert_eq!(bold.blocks[0].runs[1].text, "😀");
    assert!(bold.blocks[0].runs[1].marks.bold);
    let italic = bold.format(selected, RichFormat::Italic(true)).unwrap();
    let code = italic.format(selected, RichFormat::Code(true)).unwrap();
    assert!(code.blocks[0].runs[1].marks.italic && code.blocks[0].runs[1].marks.code);
    let linked = code
        .format(
            selected,
            RichFormat::Link(Some("https://example.test/a?x=1&y=2".into())),
        )
        .unwrap();
    assert!(linked.blocks[0].runs[1].marks.link.is_some());
    let clear = linked
        .format(selected, RichFormat::Link(None))
        .unwrap()
        .format(selected, RichFormat::Code(false))
        .unwrap()
        .format(selected, RichFormat::Italic(false))
        .unwrap()
        .format(selected, RichFormat::Bold(false))
        .unwrap();
    assert_eq!(clear, doc);
    assert_eq!(
        doc.format(
            RichSelection::collapsed(point(0, 2)),
            RichFormat::Bold(true)
        )
        .unwrap(),
        doc
    );
}
#[test]
fn explicit_budgets_links_and_canonical_failures_reject_without_output() {
    for link in [
        "javascript:alert(1)",
        "data:text/html,x",
        "https:",
        "https://a.test/a b",
        "https://a.test/\n",
    ] {
        assert!(
            RichMarks {
                link: Some(link.into()),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    for link in [
        "https://a.test/",
        "HTTP://a.test/",
        "mailto:name@example.test",
    ] {
        RichMarks {
            link: Some(link.into()),
            ..Default::default()
        }
        .validate()
        .unwrap();
    }
    assert!(
        RichMarks {
            link: Some(format!("https://a/{}", "x".repeat(MAX_RICH_LINK_BYTES))),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(RichDocument::new(vec![]).is_err());
    assert!(
        RichDocument::new(vec![RichBlock {
            kind: RichBlockKind::Paragraph,
            runs: vec![RichRun::default(); MAX_RICH_RUNS + 1]
        }])
        .is_err()
    );
    assert!(
        RichDocument::new(
            (0..=MAX_RICH_BLOCKS)
                .map(|_| RichBlock {
                    kind: Default::default(),
                    runs: vec![RichRun::default()]
                })
                .collect()
        )
        .is_err()
    );
    let doc = document("Text");
    assert!(
        doc.replace(
            RichSelection::default(),
            &"x".repeat(MAX_RICH_TEXT_BYTES + 1),
            Default::default()
        )
        .is_err()
    );
    assert!(
        doc.replace(
            RichSelection::default(),
            &"\n".repeat(MAX_RICH_BLOCKS),
            Default::default()
        )
        .is_err()
    );
    let raw = RichDocument {
        blocks: vec![RichBlock {
            kind: Default::default(),
            runs: vec![
                RichRun {
                    text: "one".into(),
                    marks: Default::default(),
                },
                RichRun {
                    text: "two".into(),
                    marks: Default::default(),
                },
            ],
        }],
    };
    assert!(raw.validate().is_err());
    let link = RichMarks {
        link: Some(format!("https://a/{}", "x".repeat(1000))),
        ..Default::default()
    };
    assert!(
        RichDocument::new(vec![RichBlock {
            kind: Default::default(),
            runs: (0..140)
                .map(|index| RichRun {
                    text: index.to_string(),
                    marks: link.clone()
                })
                .collect()
        }])
        .is_err()
    );
}
