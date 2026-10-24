use super::*;

#[test]
fn reused_frame_storage_does_not_restore_consumed_payloads_or_grow_across_small_frames() {
    let app = App::memory();
    let id = app
        .scope()
        .create(Descriptor::new(glory_render::Props::View))
        .unwrap();
    app.flush().unwrap();
    let mut stable = None;
    for frame in 0..32 {
        for n in 0..16 {
            app.scope()
                .patch(
                    id,
                    Patch::Name(glory_render::Change::Set(Some(format!("{frame}:{n}")))),
                )
                .unwrap();
        }
        app.flush().unwrap();
        app.pump().unwrap();
        assert!(app.session.present.borrow().commands.is_empty());
        assert!(app.session.command_buffer.take().is_empty());
        let capacity = app.buffer_stats();
        if frame == 3 {
            stable = Some(capacity);
        }
        if frame >= 4 {
            assert_eq!(Some(capacity), stable);
        }
    }
    let huge = 1024 * 1024 / size_of::<Mutation>() + 1;
    for _ in 0..huge {
        app.scope()
            .patch(id, Patch::Name(glory_render::Change::Clear))
            .unwrap();
    }
    app.flush().unwrap();
    assert!(app.buffer_stats().bytes <= 2 * 1024 * 1024);
    assert!(app.session.present.borrow().commands.is_empty());
    assert!(app.session.command_buffer.take().is_empty());
}

#[test]
fn returning_a_detached_batch_preserves_new_queue_contents() {
    let app = App::memory();
    let id = app
        .scope()
        .create(Descriptor::new(glory_render::Props::View))
        .unwrap();
    let mut batch = app.session.command_buffer.take();
    std::mem::swap(&mut *batch, &mut app.session.present.borrow_mut().commands);
    app.scope()
        .patch(
            id,
            Patch::Name(glory_render::Change::Set(Some("new".into()))),
        )
        .unwrap();
    let receipt = app.session.renderer.borrow_mut().apply(&batch).unwrap();
    app.session.applied.set(receipt.applied);
    drop(batch);
    assert_eq!(app.session.present.borrow().commands.len(), 1);
    app.flush().unwrap();
    assert_eq!(
        app.session
            .present
            .borrow()
            .tree
            .node(id)
            .unwrap()
            .descriptor
            .view()
            .name
            .as_deref(),
        Some("new")
    );
}

#[test]
fn failed_apply_returns_empty_storage_without_replaying_rejected_commands() {
    struct Fail;
    impl Render for Fail {
        fn capabilities(&self) -> glory_render::Capabilities {
            Default::default()
        }
        fn apply(
            &mut self,
            _: &[Mutation],
        ) -> std::result::Result<glory_render::Receipt, RenderError> {
            Err(RenderError::Unsupported("injected"))
        }
        fn request(&mut self, _: Query, _: QueryToken) -> std::result::Result<(), RenderError> {
            unreachable!()
        }
        fn drain(&mut self, _: &mut Vec<Input>) {}
    }
    let app = App::new(|_| Fail);
    app.scope()
        .create(Descriptor::new(glory_render::Props::View))
        .unwrap();
    assert_eq!(
        app.flush(),
        Err(Error::Render(RenderError::Unsupported("injected")))
    );
    assert!(app.session.present.borrow().commands.is_empty());
    assert!(app.session.command_buffer.take().is_empty());
    assert_eq!(app.flush(), Err(Error::Faulted));
}
