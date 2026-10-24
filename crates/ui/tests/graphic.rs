use glory_render::*;
use glory_ui::{App, graphic};
#[test]
fn reactive_graphics_validate_initial_and_later_payloads_before_model_changes() {
    let app = App::memory();
    let source = app.scope().cage(GraphicProps {
        width: f64::INFINITY,
        ..Default::default()
    });
    assert!(app.mount(graphic().drawing(source)).is_err());
    assert!(app.tree().roots().is_empty());
    source.revise(|value| value.width = 24.0);
    app.mount(graphic().drawing(source)).unwrap();
    let id = app.tree().roots()[0];
    source.revise(|value| value.width = -1.0);
    assert!(app.flush().is_err());
    assert_eq!(source.get().width, -1.0);
    assert!(
        matches!(app.tree().node(id).unwrap().descriptor.view().props,Props::Graphic(props) if props.width==24.0)
    );
    source.revise(|value| value.width = 48.0);
    app.flush().unwrap();
    assert!(
        matches!(app.tree().node(id).unwrap().descriptor.view().props,Props::Graphic(props) if props.width==48.0)
    );
    app.clear().unwrap();
    source.revise(|value| value.width = 64.0);
    app.flush().unwrap();
    assert!(app.tree().roots().is_empty());
}
