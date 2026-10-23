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
fn node(parent: Option<usize>, props: Props) -> PlanNode {
    PlanNode {
        parent,
        descriptor: Descriptor::new(props),
    }
}
#[test]
fn planned_and_ordinary_construction_have_identical_html_and_finite_host_operations() {
    let plan = Plan::new(vec![
        node(None, Props::View),
        node(Some(0), Props::Group),
        node(
            Some(1),
            Props::Input(InputProps {
                value: "a🙂\0\r\nb".into(),
                multiline: true,
                ..Default::default()
            }),
        ),
        node(
            Some(0),
            Props::Check(CheckProps {
                checked: true,
                label: "check".into(),
                ..Default::default()
            }),
        ),
        node(
            Some(0),
            Props::Text(TextProps {
                content: "<&>".into(),
                ..Default::default()
            }),
        ),
    ])
    .unwrap();
    let runtime = RuntimeId::new();
    let mut allocator = NodeIds::new(runtime);
    let ids: Vec<_> = plan.nodes().iter().map(|_| allocator.allocate()).collect();
    let mut planned = Mapper::new(runtime);
    let mut ordinary = Mapper::new(runtime);
    let (mut a, mut b) = (Record::default(), Record::default());
    planned
        .apply(
            &[Mutation::CreatePlan {
                plan: plan.clone(),
                ids: ids.clone(),
            }],
            &mut a,
        )
        .unwrap();
    for (index, node) in plan.nodes().iter().enumerate() {
        ordinary
            .apply(
                &[Mutation::Create {
                    id: ids[index],
                    node: Box::new(node.descriptor.clone()),
                }],
                &mut b,
            )
            .unwrap();
        if let Some(parent) = node.parent {
            ordinary
                .apply(
                    &[Mutation::Place {
                        id: ids[index],
                        parent: Some(ids[parent]),
                        before: None,
                    }],
                    &mut b,
                )
                .unwrap();
        }
    }
    let attach = Mutation::Place {
        id: ids[0],
        parent: None,
        before: None,
    };
    planned
        .apply(std::slice::from_ref(&attach), &mut a)
        .unwrap();
    ordinary.apply(&[attach], &mut b).unwrap();
    assert_eq!(a.0, b.0);
    assert_eq!(planned.html().unwrap(), ordinary.html().unwrap());
    assert_eq!(planned.tree().stats(), ordinary.tree().stats());
    assert_eq!(planned.receipt().applied, 2);
}
#[test]
fn an_internal_html_override_is_checked_before_host_work_and_keeps_the_prefix() {
    let runtime = RuntimeId::new();
    let mut allocator = NodeIds::new(runtime);
    let prefix = allocator.allocate();
    let mut paragraph = node(Some(0), Props::Text(Default::default()));
    paragraph
        .descriptor
        .options
        .insert(HtmlTextOptions::default().tag(TextTag::P));
    let plan = Plan::new(vec![
        node(None, Props::Button(Default::default())),
        paragraph,
    ])
    .unwrap();
    let ids = vec![allocator.allocate(), allocator.allocate()];
    let mut mapper = Mapper::new(runtime);
    let mut sink = Record::default();
    assert!(
        mapper
            .apply(
                &[
                    Mutation::Create {
                        id: prefix,
                        node: Box::new(Descriptor::new(Props::View))
                    },
                    Mutation::CreatePlan {
                        plan,
                        ids: ids.clone()
                    },
                ],
                &mut sink
            )
            .is_err()
    );
    assert_eq!(mapper.receipt().applied, 1);
    assert_eq!(sink.0.len(), 1);
    assert!(mapper.tree().node(ids[0]).is_err());
}
