use glory_html::{HtmlTextOptions, Mapper, Op, Sink, TextTag};
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
fn html_paragraph_override_cannot_enter_a_button_through_a_moved_or_resumed_group() {
    for resume in [false, true] {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let mut mapper = Mapper::new(runtime);
        let mut sink = Record::default();
        let button = ids.allocate();
        let group = ids.allocate();
        let text = ids.allocate();
        let mut paragraph = Descriptor::new(Props::Text(TextProps {
            content: "body".into(),
            ..Default::default()
        }));
        paragraph
            .options
            .insert(HtmlTextOptions::default().tag(TextTag::P));
        mapper
            .apply(
                &[
                    Mutation::Create {
                        id: button,
                        node: Box::new(Descriptor::new(Props::Button(ButtonProps::default()))),
                    },
                    Mutation::Place {
                        id: button,
                        parent: None,
                        before: None,
                    },
                    Mutation::Create {
                        id: group,
                        node: Box::new(Descriptor::new(Props::Group)),
                    },
                    Mutation::Place {
                        id: group,
                        parent: None,
                        before: None,
                    },
                    Mutation::Create {
                        id: text,
                        node: Box::new(paragraph),
                    },
                    Mutation::Place {
                        id: text,
                        parent: Some(group),
                        before: None,
                    },
                ],
                &mut sink,
            )
            .unwrap();
        if resume {
            mapper
                .apply(&[Mutation::Suspend { id: group }], &mut sink)
                .unwrap();
        }
        let before = mapper.html().unwrap();
        let ops = sink.0.len();
        let receipt = mapper.receipt();
        assert_eq!(
            glory_html::preflight_place(
                mapper.tree(),
                glory_render::memory::Subtree::Existing(group),
                Some(button)
            ),
            Err(Error::InvalidProps(
                "button text cannot map to an HTML paragraph"
            ))
        );
        assert_eq!(mapper.receipt(), receipt);
        assert_eq!(mapper.html().unwrap(), before);
        glory_html::preflight_place(
            mapper.tree(),
            glory_render::memory::Subtree::Existing(group),
            None,
        )
        .unwrap();
        let mutation = if resume {
            Mutation::Resume {
                id: group,
                parent: Some(button),
                before: None,
            }
        } else {
            Mutation::Place {
                id: group,
                parent: Some(button),
                before: None,
            }
        };
        assert!(matches!(
            mapper.apply(&[mutation], &mut sink),
            Err(Error::InvalidProps(_))
        ));
        assert_eq!(mapper.html().unwrap(), before);
        assert_eq!(sink.0.len(), ops);
        assert_eq!(mapper.receipt(), receipt);
        assert_eq!(mapper.tree().node(group).unwrap().parent, None);
        assert_eq!(mapper.tree().node(group).unwrap().suspended, resume);
        let restore = if resume {
            Mutation::Resume {
                id: group,
                parent: None,
                before: None,
            }
        } else {
            Mutation::Place {
                id: group,
                parent: None,
                before: None,
            }
        };
        mapper.apply(&[restore], &mut sink).unwrap();
        assert!(mapper.html().unwrap().contains(">body</p>"));
    }
}
