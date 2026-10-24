//! Server and browser use this identical resource/semantic factory.
use glory::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub const VALUE: &str =
    "</script><script>window.payloadInjected=1</script><!-- & \u{2028} \u{2029} \0中文";
pub struct State {
    pub key: Cage<u64>,
    pub profile: Resource<u64, String, String>,
    pub error: Resource<u64, String, String>,
    pub loads: Rc<Cell<usize>>,
}
fn label(state: &ResourceState<u64, String, String>) -> String {
    match state {
        ResourceState::Ready { value, .. } => value.to_string(),
        ResourceState::Failed { error, .. } => format!("failed:{error}"),
        ResourceState::Pending { key } => format!("pending:{key}"),
        ResourceState::Idle => "idle".into(),
        ResourceState::Unavailable { .. } => "unavailable".into(),
    }
}
fn ui(
    scope: &Scope,
    resources: &Resources,
) -> std::result::Result<(glory::ui::View, State), glory::resources::Error> {
    let key = scope.cage(7_u64);
    let loads = Rc::new(Cell::new(0));
    let loading = loads.clone();
    let profile = resources.bind(
        scope.owner(),
        "profile",
        "profile/1",
        move || key.get(),
        move |id| {
            loading.set(loading.get() + 1);
            async move { Ok::<_, String>(format!("client:{id}")) }
        },
    )?;
    let loading = loads.clone();
    let error = resources.bind(
        scope.owner(),
        "problem",
        "problem/1",
        || 0_u64,
        move |_| {
            loading.set(loading.get() + 1);
            async { Err::<String, _>("client error".to_owned()) }
        },
    )?;
    let widget = view().class("resource-snapshot").fill((
        text().class("snapshot-profile").content(profile.map(label)),
        text().class("snapshot-problem").content(error.map(label)),
        button()
            .class("snapshot-refresh")
            .label("Refresh")
            .on(activate, move |_| profile.refresh().expect("live profile")),
    ));
    Ok((
        widget,
        State {
            key,
            profile,
            error,
            loads,
        },
    ))
}
pub fn widget(resources: Resources, state: Rc<RefCell<Option<State>>>) -> impl Widget + use<> {
    widget(move |scope| {
        let (widget, source) = ui(scope, &resources).map_err(|error| {
            glory::ui::Error::Render(glory::render::Error::Backend(error.to_string()))
        })?;
        widget.show_in(scope)?;
        *state.borrow_mut() = Some(source);
        Ok(())
    })
}
