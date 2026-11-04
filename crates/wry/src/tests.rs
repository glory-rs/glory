use super::*;

fn message(renderer: &Renderer, payload: Value) -> String {
    let mut value = json!({"v":protocol::VERSION,"session":renderer.session()});
    value
        .as_object_mut()
        .unwrap()
        .extend(payload.as_object().unwrap().clone());
    value.to_string()
}
fn ready(renderer: &Renderer) {
    renderer
        .receive(&message(renderer, json!({"type":"ready"})))
        .unwrap();
}
fn batch(renderer: &Renderer) -> Value {
    let script = renderer.take_script().unwrap().unwrap();
    serde_json::from_str(
        script
            .strip_prefix("window.__gloryWryReceive(")
            .unwrap()
            .strip_suffix(");")
            .unwrap(),
    )
    .unwrap()
}
fn ack(renderer: &Renderer, value: &Value) {
    renderer
        .receive(&message(
            renderer,
            json!({"type":"ack","sequence":value["sequence"]}),
        ))
        .unwrap();
}
#[test]
fn author_styles_follow_document_order_without_rebuilding_or_parsing_markup() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    create(
        &mut renderer,
        id,
        Props::Text(TextProps {
            content: "retained".into(),
            ..Default::default()
        }),
    );
    let css = ".theme { color: #123456; } /* </style><script>literal</script> */";
    renderer.set_author_css(css).unwrap();
    assert!(renderer.take_script().unwrap().is_none());
    ready(&renderer);
    let first = batch(&renderer);
    assert_eq!(first["commands"][0]["op"], "create");
    assert_eq!(first["commands"][2], json!({"op":"css","value":css}));
    renderer
        .set_author_css(".theme { color: #abcdef }")
        .unwrap();
    assert!(renderer.take_script().unwrap().is_none());
    ack(&renderer, &first);
    let second = batch(&renderer);
    assert_eq!(second["commands"].as_array().unwrap().len(), 1);
    assert_eq!(second["commands"][0]["op"], "css");
    // A rejected oversized declaration does not poison the renderer or enqueue a prefix.
    assert!(
        renderer
            .set_author_css(&"x".repeat(1024 * 1024 + 1))
            .is_err()
    );
    ack(&renderer, &second);
    assert!(renderer.take_script().unwrap().is_none());
    renderer.set_author_css("").unwrap();
    assert_eq!(batch(&renderer)["commands"][0]["value"], "");
}
fn create(renderer: &mut Renderer, id: NodeId, props: Props) -> Receipt {
    renderer
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(props)),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap()
}
#[test]
fn batches_wait_for_ready_and_ack_and_never_serialize_full_snapshots() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    create(
        &mut renderer,
        id,
        Props::Text(TextProps {
            content: "</script>中文\u{2028}".into(),
            ..Default::default()
        }),
    );
    assert!(renderer.take_script().unwrap().is_none());
    ready(&renderer);
    let first = batch(&renderer);
    assert_eq!(first["commands"][0]["op"], "create");
    assert_eq!(
        first["commands"][0]["shape"]["pieces"][1]["kind"]["text"],
        "</script>中文\u{2028}"
    );
    renderer
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Text(Change::Set("next".into())),
        }])
        .unwrap();
    assert!(renderer.take_script().unwrap().is_none());
    ack(&renderer, &first);
    let second = batch(&renderer);
    assert_eq!(second["commands"].as_array().unwrap().len(), 1);
    assert_eq!(second["commands"][0]["op"], "patch");
    assert!(second["commands"][0].get("shape").is_none());
    assert!(
        renderer
            .receive(&message(
                &renderer,
                json!({"type":"ack","sequence":first["sequence"]})
            ))
            .is_err()
    );
}
#[test]
fn query_replies_keep_original_barriers_and_complete_once() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let first = create(&mut renderer, id, Props::Input(InputProps::default()));
    renderer
        .request(Query::Focus(id), QueryToken(u64::MAX))
        .unwrap();
    renderer
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Name(Change::Set(Some("new".into()))),
        }])
        .unwrap();
    let reply = message(
        &renderer,
        json!({"type":"response","token":u64::MAX.to_string(),"value":true}),
    );
    assert!(renderer.receive(&reply).unwrap());
    assert!(!renderer.receive(&reply).unwrap());
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert_eq!(
        inputs,
        vec![Input::Response {
            token: QueryToken(u64::MAX),
            applied: first.applied,
            result: Ok(QueryValue::Focus(true))
        }]
    );
    renderer.request(Query::Bounds(id), QueryToken(5)).unwrap();
    renderer
        .receive(&message(
            &renderer,
            json!({"type":"response","token":"5","value":{"x":0,"y":0,"width":-1,"height":2}}),
        ))
        .unwrap();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.last(),
        Some(Input::Response {
            result: Err(Error::InvalidResponse(_)),
            ..
        })
    ));
}
#[test]
fn disposal_releases_waiters_and_recycled_nodes_reject_old_events() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let old = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    create(&mut renderer, old, Props::Button(ButtonProps::default()));
    renderer.request(Query::Bounds(old), QueryToken(7)).unwrap();
    renderer.apply(&[Mutation::Dispose { id: old }]).unwrap();
    ids.release(old);
    let next = ids.allocate();
    assert_eq!(old.slot(), next.slot());
    create(&mut renderer, next, Props::Button(ButtonProps::default()));
    assert!(
        !renderer
            .receive(&message(
                &renderer,
                json!({"type":"event","node":protocol::id(old),"event":{"kind":"activate"}})
            ))
            .unwrap()
    );
    assert!(
        !renderer
            .receive(&message(
                &renderer,
                json!({"type":"response","token":"7","value":null})
            ))
            .unwrap()
    );
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert_eq!(inputs.len(), 1);
    assert!(matches!(inputs[0],Input::Response{result:Err(Error::NodeGone(node)),..} if node==old));
    assert!(renderer.0.borrow().pending.is_empty());
}
#[test]
fn malformed_foreign_stale_and_future_inputs_never_reenter_business_code() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    create(
        &mut renderer,
        id,
        Props::Input(InputProps {
            revision: 3,
            ..Default::default()
        }),
    );
    let edit = |revision: &str| {
        message(
            &renderer,
            json!({"type":"event","node":protocol::id(id),"event":{"kind":"value","value":"中文😀","revision":revision,"composing":false}}),
        )
    };
    assert!(!renderer.receive(&edit("2")).unwrap());
    assert!(matches!(
        renderer.receive(&edit("4")),
        Err(Error::InputRevision { .. })
    ));
    assert!(renderer.receive(&edit("03")).is_err());
    let foreign = Renderer::new(runtime);
    assert!(!foreign.receive(&edit("3")).unwrap());
    renderer.receive(&edit("3")).unwrap();
    assert_eq!(renderer.0.borrow().inputs.len(), 1);
    assert!(
        renderer
            .receive(&"x".repeat(protocol::MAX_IPC_BYTES + 1))
            .is_err()
    );
    renderer.close();
    assert!(!renderer.receive(&edit("3")).unwrap());
    assert!(renderer.take_script().is_err());
}
#[test]
fn preflight_and_direct_apply_reject_unsupported_metadata_with_accepted_prefix() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let bad = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let unsupported = Descriptor::new(Props::Select(SelectProps::default()));
    assert!(renderer.preflight(unsupported.view(), None).is_err());
    assert!(
        renderer
            .apply(&[
                Mutation::Create {
                    id,
                    node: Box::new(Descriptor::new(Props::View))
                },
                Mutation::Create {
                    id: bad,
                    node: Box::new(unsupported)
                }
            ])
            .is_err()
    );
    assert!(renderer.0.borrow().mapper.tree().node(id).is_ok());
    assert!(renderer.0.borrow().mapper.tree().node(bad).is_err());
    let applied = renderer.0.borrow().mapper.receipt().applied;
    assert!(applied > 0);
    assert_eq!(renderer.apply(&[]).unwrap().applied, applied);
    assert!(renderer.listen(id, EventKind::Drag, true).is_err());
    renderer.listen(id, EventKind::Drag, false).unwrap();
}
#[test]
fn selection_boundaries_validate_before_transport_changes() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    create(
        &mut renderer,
        id,
        Props::Input(InputProps {
            value: "中😀a".into(),
            ..Default::default()
        }),
    );
    let size = renderer.0.borrow().transport.commands.len();
    let action = |start, end| Action::SelectText {
        node: id,
        revision: 0,
        selection: TextSelection {
            start,
            end,
            ..Default::default()
        },
    };
    assert!(renderer.action(action(1, 2)).is_err());
    assert_eq!(renderer.0.borrow().transport.commands.len(), size);
    renderer.action(action(1, 3)).unwrap();
}

#[test]
fn text_receipts_follow_the_ui_commit_and_reset_rejected_program_barriers() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    create(
        &mut renderer,
        id,
        Props::Input(InputProps {
            value: "initial".into(),
            revision: 3,
            ..Default::default()
        }),
    );
    ready(&renderer);
    let initial = batch(&renderer);
    ack(&renderer, &initial);
    let edit = message(
        &renderer,
        json!({"type":"event","node":protocol::id(id),"event":{"kind":"value","value":"typed ahead","revision":"3","composing":false}}),
    );
    assert!(renderer.receive(&edit).unwrap());
    assert!(
        renderer.take_script().unwrap().is_none(),
        "a queued proposal is not completed input"
    );
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert_eq!(inputs.len(), 1);
    renderer
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Value {
                value: Change::Set("typed ahead".into()),
                revision: 4,
            },
        }])
        .unwrap();
    let updated = batch(&renderer);
    assert_eq!(
        updated["commands"].as_array().unwrap().last().unwrap(),
        &json!({"op":"input_done","id":protocol::id(id),"revision":"4","reset":false})
    );
    ack(&renderer, &updated);
    assert!(
        !renderer.receive(&edit).unwrap(),
        "old program barriers remain rejected"
    );
    let rejected = batch(&renderer);
    assert_eq!(
        rejected["commands"][0],
        json!({"op":"input_done","id":protocol::id(id),"revision":"4","reset":true,"value":"typed ahead"})
    );
    assert!(renderer.0.borrow().inputs.is_empty());
    ack(&renderer, &rejected);
    assert!(renderer.0.borrow().edit_receipts.is_empty());
}
