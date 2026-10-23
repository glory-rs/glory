use glory::prelude::*;
use glory::render::{Descriptor, EventKind, NodeId, Props, Query};
use std::{cell::Cell, rc::Rc};

pub type Values = (Cage<usize>, Bond<usize>);
pub struct State {
    pub model: Cage<(usize, Vec<u8>)>,
    pub last: Rc<Cell<Option<Values>>>,
    pub sent: Rc<Cell<usize>>,
    pub called: Rc<Cell<usize>>,
    pub dropped: Rc<Cell<usize>>,
}
struct Capture(Rc<Cell<usize>>);
impl Drop for Capture {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

pub fn ui(scope: &glory::ui::Scope) -> (impl Widget + use<>, State) {
    let model = scope.cage((0usize, vec![7u8; 1024 * 1024]));
    let shown = scope.cage(true);
    let target = Rc::new(Cell::new(None::<NodeId>));
    let input = target.clone();
    let last = Rc::new(Cell::new(None));
    let out = last.clone();
    let sent = Rc::new(Cell::new(0));
    let sends = sent.clone();
    let called = Rc::new(Cell::new(0));
    let calls = called.clone();
    let dropped = Rc::new(Cell::new(0));
    let drops = dropped.clone();
    let widget = view().fill((
        component(move |scope| {
            let mut descriptor = Descriptor::new(Props::Input(Default::default()));
            descriptor.classes.push("query-target".into());
            input.set(Some(scope.create(descriptor)?));
            Ok(())
        }),
        text()
            .class("query-count")
            .content(model.map(|v| v.0.to_string())),
        button()
            .class("query-toggle")
            .fill(text().content("Toggle requester"))
            .on(EventKind::Activate, move |_| shown.revise(|v| *v = !*v)),
        when(shown, move || {
            out.set(None);
            let (out, sends, calls, drops, target) = (
                out.clone(),
                sends.clone(),
                calls.clone(),
                drops.clone(),
                target.clone(),
            );
            component(move |scope| {
                let requester = scope.clone();
                let (out, sends, calls, drops) =
                    (out.clone(), sends.clone(), calls.clone(), drops.clone());
                let target = target.get().expect("target built before requester");
                let mut_derived = Rc::new(Cell::new(None));
                button()
                    .class("query-send")
                    .fill(text().content("Query external input"))
                    .on(EventKind::Activate, move |_| {
                        sends.set(sends.get() + 1);
                        let capture = Capture(drops.clone());
                        let (out, calls, cached) =
                            (out.clone(), calls.clone(), mut_derived.clone());
                        requester
                            .request(Query::Bounds(target), move |result| {
                                let _capture = capture;
                                result.expect("live external target");
                                let field = model.field("count", |v| &v.0, |v| &mut v.0);
                                let derived = cached.get().unwrap_or_else(|| {
                                    let derived = field.map(|v| *v);
                                    cached.set(Some(derived));
                                    derived
                                });
                                field.revise(|v| *v += 1);
                                out.set(Some((field, derived)));
                                calls.set(calls.get() + 1);
                            })
                            .expect("live requester");
                    })
                    .show_in(scope)
            })
        }),
    ));
    (
        widget,
        State {
            model,
            last,
            sent,
            called,
            dropped,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_releases_query_form_resources_without_sending_host_queries() {
        let mut state = None;
        let snapshot = glory::ssr::snapshot_with(|scope| {
            let (widget, value) = ui(scope);
            state = Some(value);
            widget
        })
        .unwrap();
        let state = state.unwrap();
        assert!(snapshot.html.contains("query-target") && snapshot.html.contains("query-send"));
        assert_eq!(
            (state.sent.get(), state.called.get(), state.dropped.get()),
            (0, 0, 0)
        );
        assert!(!state.model.is_alive());
    }
}
