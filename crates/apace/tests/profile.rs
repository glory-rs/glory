#![cfg(all(not(target_arch = "wasm32"), feature = "raster"))]
use glory_apace::{Raster, Renderer};
use glory_render::profile::{Phase, Profile};
use glory_ui::{App, text};

#[test]
fn shared_profile_measures_layout_paint_and_changed_draw_without_dirtying_idle_frames() {
    let mut renderer = None;
    let app = App::new(|id| {
        let value = Renderer::new(id);
        renderer = Some(value.clone());
        value
    });
    let renderer = renderer.unwrap();
    renderer.set_viewport(160, 120, 1.).unwrap();
    let profile = Profile::native();
    app.set_profile(Some(profile.clone()));
    let message = app.scope().cage("initial".to_owned());
    app.mount(text().content(message)).unwrap();
    let mut raster = Raster::new();
    assert!(raster.draw(&renderer).unwrap().rendered);
    let snapshot = profile.snapshot();
    for phase in [
        Phase::Build,
        Phase::Bind,
        Phase::Commit,
        Phase::Layout,
        Phase::Paint,
        Phase::Draw,
        Phase::Style,
        Phase::Damage,
        Phase::Construct,
        Phase::PostConstruct,
        Phase::BoxLayout,
        Phase::Transform,
        Phase::PaintTree,
        Phase::LayoutCleanup,
        Phase::Hover,
        Phase::Subdocuments,
        Phase::TextShape,
        Phase::TextBreak,
        Phase::TextAlign,
    ] {
        assert!(snapshot.sample(phase).calls > 0, "{phase:?}");
        assert_eq!(snapshot.sample(phase).invalid_clock, 0);
    }
    assert!(snapshot.sample(Phase::TextShape).units >= "initial".len() as u64);
    let revision = renderer.stats().visual_revision;
    assert!(profile.reset());
    for _ in 0..5 {
        app.pump().unwrap();
        assert!(!raster.draw(&renderer).unwrap().rendered);
    }
    assert_eq!(renderer.stats().visual_revision, revision);
    for phase in Phase::ALL {
        if matches!(phase, Phase::Observe | Phase::Resources | Phase::Flush) {
            continue;
        }
        assert_eq!(profile.snapshot().sample(phase).calls, 0, "{phase:?}");
    }
    message.revise(|value| *value = "changed".into());
    app.pump().unwrap();
    assert!(raster.draw(&renderer).unwrap().rendered);
    assert_eq!(profile.snapshot().sample(Phase::Paint).calls, 1);
    assert_eq!(profile.snapshot().sample(Phase::Draw).calls, 1);
    assert!(profile.snapshot().sample(Phase::TextShape).calls > 0);
    app.set_profile(None);
    let snapshot = profile.snapshot();
    message.revise(|value| *value = "unprofiled".into());
    app.pump().unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(profile.snapshot(), snapshot);
}

#[test]
fn profiling_attachment_and_reset_do_not_dirty_a_clean_document_or_reuse_old_intervals() {
    let runtime = glory_render::RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let mut raster = Raster::new();
    let profile = Profile::native();
    let mut ids = glory_render::NodeIds::new(runtime);
    let node = ids.allocate();
    use glory_render::{Descriptor, Mutation, Props, Render};
    renderer
        .apply(&[
            Mutation::Create {
                id: node,
                node: Box::new(Descriptor::new(Props::Text(glory_render::TextProps {
                    content: "cached native text".into(),
                    ..Default::default()
                }))),
            },
            Mutation::Place {
                id: node,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    raster.draw(&renderer).unwrap();
    let revision = renderer.stats().visual_revision;
    assert!(renderer.set_profile(Some(profile.clone())).is_none());
    let retained = profile.recording();
    assert!(profile.reset());
    assert!(!raster.draw(&renderer).unwrap().rendered);
    assert_eq!(renderer.stats().visual_revision, revision);
    for phase in Phase::ALL {
        if phase == Phase::Resources {
            continue;
        }
        assert_eq!(profile.snapshot().sample(phase).calls, 0, "{phase:?}");
    }
    assert!(!retained.record(
        Phase::Style,
        glory_render::profile::Sample {
            calls: 1,
            units: 1,
            ..Default::default()
        }
    ));
    let before = profile.snapshot();
    renderer.set_profile(None);
    assert_eq!(profile.snapshot(), before);
}

#[test]
fn engine_counters_belong_to_documents_even_when_the_runtime_identity_is_shared() {
    use glory_render::{Descriptor, Mutation, NodeIds, Props, Render, RuntimeId, TextProps};
    let runtime = RuntimeId::new();
    let mut a = Renderer::new(runtime);
    let mut b = Renderer::new(runtime);
    let pa = Profile::native();
    let pb = Profile::native();
    a.set_profile(Some(pa.clone()));
    b.set_profile(Some(pb.clone()));
    let create = |renderer: &mut Renderer, content: &str| {
        let mut ids = NodeIds::new(runtime);
        let node = ids.allocate();
        renderer
            .apply(&[
                Mutation::Create {
                    id: node,
                    node: Box::new(Descriptor::new(Props::Text(TextProps {
                        content: content.into(),
                        ..Default::default()
                    }))),
                },
                Mutation::Place {
                    id: node,
                    parent: None,
                    before: None,
                },
            ])
            .unwrap();
    };
    create(&mut a, "document alpha");
    create(&mut b, "document beta");
    let mut raster = Raster::new();
    raster.draw(&a).unwrap();
    let first = pa.snapshot();
    assert!(first.sample(Phase::TextShape).calls > 0);
    raster.draw(&b).unwrap();
    assert_eq!(pa.snapshot(), first);
    let second = pb.snapshot();
    assert!(second.sample(Phase::TextShape).calls > 0);
    assert!(pa.reset());
    // Re-recording a clean document after another document used the pixel
    // buffer is paint/draw work, not another style/shape/layout resolution.
    raster.draw(&a).unwrap();
    assert_eq!(pa.snapshot().sample(Phase::Style).calls, 0);
    assert_eq!(pa.snapshot().sample(Phase::TextShape).calls, 0);
    assert_eq!(pb.snapshot(), second);
}
