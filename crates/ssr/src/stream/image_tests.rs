use super::*;
use glory_render::EventKind;
use glory_ui::image;

#[test]
fn image_source_revision_and_real_src_delta_survive_stream_patch_and_end_without_status_callbacks()
{
    let gate = Gate::default();
    let loader = gate.clone();
    let observed = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = observed.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "image-source",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let source = resource.map(|state| {
                if matches!(state, ResourceState::Ready { .. }) {
                    "ready.png".to_string()
                } else {
                    "pending.png".to_string()
                }
            });
            image()
                .source(source)
                .alt("Photo")
                .on(EventKind::ImageStatus, move |_| seen.set(seen.get() + 1))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("src=\"pending.png\""));
    assert_eq!(observed.get(), 0);
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(patch.body().contains("\"edit\":\"revision\",\"value\":2"));
    assert!(patch.body().contains("\"name\":\"src\""));
    assert!(patch.body().contains("ready.png"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    assert_eq!(observed.get(), 0);
}
