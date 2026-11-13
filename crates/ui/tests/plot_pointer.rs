use glory_render::{memory::Memory, *};
use glory_ui::{App, Widget, view};
use std::cell::RefCell;
use std::rc::Rc;

fn event(node: NodeId, phase: PlotPointerPhase) -> Event {
    Event::PlotPointer {
        from: node,
        pointer: PlotPointer {
            pointer_id: 1,
            device: PlotPointerDevice::Mouse,
            phase,
            x: 10.375,
            y: 20.625,
            viewport: Rect {
                width: 480.,
                height: 300.,
                ..Rect::default()
            },
        },
    }
}

#[test]
fn coordinate_channel_is_direct_and_independent_of_legacy_pointer_boundaries() {
    let app = App::memory();
    let coordinates = Rc::new(RefCell::new(vec![]));
    let old = Rc::new(RefCell::new(vec![]));
    let seen = coordinates.clone();
    let boundaries = old.clone();
    let mounted = view()
        .on(EventKind::PlotPointer, move |event| {
            seen.borrow_mut().push(event)
        })
        .on(EventKind::Pointer, move |event| {
            boundaries.borrow_mut().push(event)
        })
        .mount(app.scope())
        .unwrap();
    app.flush().unwrap();
    let id = mounted.roots()[0];
    assert!(app.scope().supports_plot_pointer().unwrap());
    let expected = event(id, PlotPointerPhase::Move);
    assert!(app.dispatch(id, expected.clone()).unwrap());
    assert_eq!(*coordinates.borrow(), [expected]);
    assert!(old.borrow().is_empty());
    let boundary = Event::Pointer {
        from: id,
        related: None,
        phase: PointerPhase::Enter,
    };
    assert!(app.dispatch(id, boundary.clone()).unwrap());
    assert_eq!(*old.borrow(), [boundary]);
    assert_eq!(coordinates.borrow().len(), 1);
    drop(mounted);
    app.flush().unwrap();
    assert!(
        !app.dispatch(id, event(id, PlotPointerPhase::Cancel))
            .unwrap()
    );
    assert_eq!(coordinates.borrow().len(), 1);
}

struct UnsupportedHost {
    memory: Memory,
    input: bool,
}
impl Render for UnsupportedHost {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            input: self.input,
            ..Default::default()
        }
    }
    fn apply(&mut self, changes: &[Mutation]) -> Result<Receipt, Error> {
        self.memory.apply(changes)
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        self.memory.request(query, token)
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        self.memory.drain(inputs);
    }
}

#[test]
fn unsupported_input_host_rejects_interest_before_replacing_handlers_or_queue() {
    let app = App::new(|runtime| UnsupportedHost {
        memory: Memory::new(runtime),
        input: true,
    });
    let mounted = view().mount(app.scope()).unwrap();
    let id = mounted.roots()[0];
    let before = app.stats();
    assert!(!app.scope().supports_plot_pointer().unwrap());
    assert!(matches!(
        app.scope()
            .on(id, EventKind::PlotPointer, |_| panic!("unregistered")),
        Err(glory_ui::Error::Render(Error::Unsupported(_)))
    ));
    assert_eq!(app.stats(), before);
    app.flush().unwrap();
    assert!(matches!(
        app.dispatch(id, event(id, PlotPointerPhase::Move)),
        Err(glory_ui::Error::Render(Error::Unsupported(_)))
    ));
}

#[test]
fn static_host_can_record_adoption_interest_but_cannot_deliver_coordinate_input() {
    let app = App::new(|runtime| UnsupportedHost {
        memory: Memory::new(runtime),
        input: false,
    });
    let mounted = view()
        .on(EventKind::PlotPointer, |_| panic!("static input"))
        .mount(app.scope())
        .unwrap();
    let id = mounted.roots()[0];
    app.flush().unwrap();
    assert!(matches!(
        app.dispatch(id, event(id, PlotPointerPhase::Move)),
        Err(glory_ui::Error::Render(Error::Unsupported(_)))
    ));
    assert!(app.scope().on(id, EventKind::Activate, |_| {}).is_ok());
    assert!(app.dispatch(id, Event::Activate).unwrap());
}
