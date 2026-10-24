use glory_render::memory::Memory;
use glory_render::*;

fn image() -> Descriptor {
    Descriptor::new(Props::Image(ImageProps {
        source: "a.png".into(),
        revision: 1,
        alternative: Some("Photo".into()),
    }))
}
fn source(value: &str, revision: u64) -> Patch {
    Patch::Source {
        source: Change::Set(value.into()),
        revision,
    }
}
#[test]
fn raw_image_revisions_are_monotonic_and_equal_requires_canonical_equal_source() {
    let mut descriptor = image();
    let initial = descriptor.clone();
    for patch in [source("a.png", 0), source("b.png", 1)] {
        assert!(matches!(
            descriptor.patch(patch),
            Err(Error::ImageRevision { .. })
        ));
        assert_eq!(descriptor, initial);
    }
    descriptor.patch(source("b\0.png", 2)).unwrap();
    descriptor.patch(source("b\u{fffd}.png", 2)).unwrap();
    descriptor
        .patch(Patch::Source {
            source: Change::Keep,
            revision: 0,
        })
        .unwrap();
    let Props::Image(value) = &descriptor.props else {
        panic!()
    };
    assert_eq!((&*value.source, value.revision), ("b\u{fffd}.png", 2));
    descriptor
        .patch(Patch::Source {
            source: Change::Clear,
            revision: 3,
        })
        .unwrap();
    let Props::Image(value) = &descriptor.props else {
        panic!()
    };
    assert_eq!((&*value.source, value.revision), ("", 3));
}
#[test]
fn prepared_image_patch_rollback_and_rejection_preserve_model_receipt_and_stats() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(image()),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let stats = memory.stats();
    assert!(memory.prepare_patch(id, &source("b.png", 1)).is_err());
    assert_eq!(memory.stats(), stats);
    {
        let prepared = memory.prepare_patch(id, &source("b.png", 2)).unwrap();
        let Props::Image(value) = prepared.descriptor().props else {
            panic!()
        };
        assert_eq!(value.source, "b.png");
    }
    assert_eq!(memory.node(id).unwrap().descriptor, image());
    assert_eq!(memory.stats(), stats);
    memory
        .prepare_patch(id, &source("b.png", 2))
        .unwrap()
        .commit();
    let Props::Image(value) = memory.node(id).unwrap().descriptor.view().props else {
        panic!()
    };
    assert_eq!(value.revision, 2);
}
#[test]
fn completion_target_and_future_revision_are_checked_before_delivery() {
    let image = image();
    let id = NodeIds::new(RuntimeId::new()).allocate();
    assert!(EventKind::ImageStatus.validate_target(&image).is_ok());
    assert!(
        EventKind::ImageStatus
            .validate_target(&Descriptor::default())
            .is_err()
    );
    assert!(
        Event::ImageStatus {
            revision: 1,
            status: ImageStatus::Loaded
        }
        .validate_payload(id, &image)
        .is_ok()
    );
    assert!(matches!(
        Event::ImageStatus {
            revision: 2,
            status: ImageStatus::Failed
        }
        .validate_payload(id, &image),
        Err(Error::ImageRevision {
            current: 1,
            incoming: 2
        })
    ));
}
