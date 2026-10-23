use std::cell::RefCell;
use std::rc::Rc;

use glory_render::memory::Memory;
use glory_render::*;
use glory_style::{Css, Length, Profile, Property, Style, Support, Unsupported, Value};
use glory_ui::{App, Error as UiError, Widget, text, view};

struct Limited {
    memory: Memory,
    profile: Profile,
    applied: Rc<RefCell<Vec<Mutation>>>,
}
impl Render for Limited {
    fn style_profile(&self) -> Profile {
        self.profile
    }
    fn capabilities(&self) -> Capabilities {
        self.memory.capabilities()
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        self.applied.borrow_mut().extend_from_slice(mutations);
        self.memory.apply(mutations)
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        self.memory.request(query, token)
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        self.memory.drain(inputs)
    }
}
fn app(support: Support) -> (App, Rc<RefCell<Vec<Mutation>>>) {
    let applied = Rc::new(RefCell::new(Vec::new()));
    let trace = applied.clone();
    (
        App::new(move |id| Limited {
            memory: Memory::new(id),
            profile: Profile {
                name: "limited",
                version: 7,
                inline: support,
                css: Css::None,
            },
            applied: trace,
        }),
        applied,
    )
}

#[test]
fn unsupported_creates_never_allocate_a_node_or_reach_the_renderer() {
    let (app, applied) = app(Support::NONE);
    assert_eq!(app.style_profile().name, "limited");
    assert_eq!(app.scope().style_profile().unwrap(), app.style_profile());
    assert_eq!(
        app.mount(view().style(Style::new().width(Length::Px(1.0))))
            .unwrap_err(),
        UiError::Render(Error::UnsupportedStyle {
            profile: "limited",
            version: 7,
            requirement: Unsupported {
                property: Property::Width,
                value: None
            },
        })
    );
    assert_eq!(app.stats().nodes, 0);
    assert_eq!(app.stats().node_slots, 0);
    assert!(applied.borrow().is_empty());
    app.mount(text().content("no inline styles")).unwrap();
}

#[test]
fn rejected_values_preserve_committed_style_and_bindings_can_recover() {
    let support = Support::ALL.without_value(Value::Percent);
    let (app, applied) = app(support);
    let style = app.scope().cage(Style::new().width(Length::Px(10.0)));
    app.mount(view().style(style)).unwrap();
    let node = app.tree().roots()[0];
    applied.borrow_mut().clear();
    style.revise(|style| style.width = Some(Length::Percent(50.0)));
    assert!(matches!(
        app.flush(),
        Err(UiError::Render(Error::UnsupportedStyle {
            requirement: Unsupported {
                property: Property::Width,
                value: Some(Value::Percent)
            },
            ..
        }))
    ));
    assert!(applied.borrow().is_empty());
    assert_eq!(
        app.tree().node(node).unwrap().descriptor.style.width,
        Some(Length::Px(10.0))
    );
    style.revise(|style| style.width = Some(Length::Px(20.0)));
    app.flush().unwrap();
    assert_eq!(
        app.tree().node(node).unwrap().descriptor.style.width,
        Some(Length::Px(20.0))
    );
    assert!(matches!(
        applied.borrow().as_slice(),
        [Mutation::Patch {
            patch: Patch::Style(_),
            ..
        }]
    ));
    app.scope()
        .patch(node, Patch::Style(Change::Clear))
        .unwrap();
    app.flush().unwrap();
    assert_eq!(
        app.tree().node(node).unwrap().descriptor.style,
        Style::new()
    );
    let invalid = Style::new().width(Length::Px(f64::NAN));
    assert!(matches!(
        app.scope()
            .patch(node, Patch::Style(Change::Set(Box::new(invalid.clone())))),
        Err(UiError::Render(Error::InvalidProps(_)))
    ));
    let mounted = text().mount(app.scope()).unwrap();
    let retired = mounted.roots()[0];
    drop(mounted);
    assert_eq!(
        app.scope()
            .patch(retired, Patch::Style(Change::Set(Box::new(invalid))))
            .unwrap_err(),
        UiError::Render(Error::NodeGone(retired))
    );
}
