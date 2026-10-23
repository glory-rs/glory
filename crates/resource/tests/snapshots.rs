use glory_core::{ResourceState, Runtime};
use glory_resource::{Error, Limits, Resources, Snapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct User {
    name: String,
}
fn seed(name: &str, value: &str) -> Resources {
    let mut snapshot = Snapshot::default();
    snapshot
        .seed(
            name,
            "user/1",
            &42_u64,
            Ok::<_, &String>(&User { name: value.into() }),
        )
        .unwrap();
    Resources::restore(snapshot).unwrap()
}

#[test]
fn explicitly_named_non_clone_results_round_trip_and_suppress_refetch() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let resources = seed("profile", "request A");
    let profile = resources
        .bind(
            &owner,
            "profile",
            "user/1",
            || 42_u64,
            |_| async {
                Ok::<_, String>(User {
                    name: "loaded".into(),
                })
            },
        )
        .unwrap();
    runtime.flush().unwrap();
    assert!(
        matches!(profile.get(), ResourceState::Ready { value, .. } if value.name == "request A")
    );
    assert_eq!(resources.remaining(), 0);
    let json = resources.snapshot().unwrap().to_json().unwrap();
    assert!(!json.contains("RuntimeId"));
    let restored =
        Resources::restore(Snapshot::from_json(&json, Limits::default()).unwrap()).unwrap();
    let client = Runtime::new();
    let client_owner = client.owner();
    let profile = restored
        .bind(
            &client_owner,
            "profile",
            "user/1",
            || 42_u64,
            |_| async {
                Ok::<_, String>(User {
                    name: "network".into(),
                })
            },
        )
        .unwrap();
    client.flush().unwrap();
    assert!(
        matches!(profile.get(), ResourceState::Ready { value, .. } if value.name == "request A")
    );
    assert_eq!(client.stats().tasks, 0);
    owner.dispose();
    assert!(resources.is_empty());
    assert!(resources.snapshot().unwrap().is_empty());
}

#[test]
fn contexts_isolate_requests_and_reject_accidentally_sharing_one_across_runtimes() {
    let a = Runtime::new();
    let ao = a.owner();
    let b = Runtime::new();
    let bo = b.owner();
    let ra = seed("profile", "A");
    let rb = seed("profile", "B");
    let bind = |r: &Resources, o| {
        r.bind(
            o,
            "profile",
            "user/1",
            || 42_u64,
            |_| async { Err::<User, _>("network".to_owned()) },
        )
    };
    let av = bind(&ra, &ao).unwrap();
    let bv = bind(&rb, &bo).unwrap();
    a.flush().unwrap();
    b.flush().unwrap();
    assert!(matches!(av.get(), ResourceState::Ready { value, .. } if value.name == "A"));
    assert!(matches!(bv.get(), ResourceState::Ready { value, .. } if value.name == "B"));
    let error = ra.bind(
        &bo,
        "another",
        "user/1",
        || 0_u64,
        |_| async { Ok::<_, String>(0) },
    );
    assert!(matches!(error, Err(Error::WrongRuntime)));
    assert!(matches!(bind(&ra, &ao), Err(Error::Duplicate(_))));
    assert_eq!(ra.len(), 1);
    assert_eq!(rb.len(), 1);
}

#[test]
fn schemas_and_bad_types_fail_before_allocating_and_retries_keep_the_seed() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let resources = seed("profile", "A");
    let wrong = resources.bind(
        &owner,
        "profile",
        "user/2",
        || 42_u64,
        |_| async { Ok::<_, String>(User { name: "".into() }) },
    );
    assert!(matches!(wrong, Err(Error::Schema { .. })));
    assert_eq!(runtime.stats().nodes, 0);
    assert_eq!(resources.remaining(), 1);
    assert!(resources.is_empty());
    let wrong_type = resources.bind(
        &owner,
        "profile",
        "user/1",
        || 42_u64,
        |_| async { Ok::<_, String>(123_u64) },
    );
    assert!(matches!(wrong_type, Err(Error::Json(_))));
    assert_eq!(runtime.stats().nodes, 0);
    assert_eq!(resources.remaining(), 1);
    resources
        .bind(
            &owner,
            "profile",
            "user/1",
            || 42_u64,
            |_| async { Ok::<_, String>(User { name: "".into() }) },
        )
        .unwrap();
    runtime.flush().unwrap();
    assert_eq!(resources.remaining(), 0);
}

#[test]
fn failed_business_results_restore_but_idle_and_unavailable_do_not_export() {
    let mut snapshot = Snapshot::default();
    snapshot
        .seed(
            "business",
            "v1",
            &0_u32,
            Err::<&u64, _>(&"missing".to_owned()),
        )
        .unwrap();
    let resources = Resources::restore(snapshot).unwrap();
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let failed = resources
        .bind(
            &owner,
            "business",
            "v1",
            || 0_u32,
            |_| async { Ok::<_, String>(0_u64) },
        )
        .unwrap();
    let pending = resources
        .bind(
            &owner,
            "pending",
            "v1",
            || 0,
            |_| std::future::pending::<Result<u64, String>>(),
        )
        .unwrap();
    assert_eq!(resources.snapshot().unwrap().len(), 1); // Idle omitted.
    runtime.flush().unwrap();
    assert!(matches!(failed.get(), ResourceState::Failed { error, .. } if *error == "missing"));
    assert!(matches!(pending.get(), ResourceState::Unavailable { .. }));
    assert_eq!(resources.snapshot().unwrap().len(), 1);
    failed.cancel().unwrap();
    assert!(resources.snapshot().unwrap().is_empty());
}

#[test]
fn row_churn_discards_live_registration_instead_of_accumulating_visited_names() {
    let resources = Resources::default();
    let runtime = Runtime::new();
    let root = runtime.owner();
    for key in 0..2000 {
        let owner = root.child();
        resources
            .bind(
                &owner,
                &format!("row/{key}"),
                "v1",
                move || key,
                |_| std::future::pending::<Result<u64, String>>(),
            )
            .unwrap();
        assert_eq!(resources.len(), 1);
        runtime.flush().unwrap();
        drop(owner);
        assert!(resources.is_empty());
        assert!(resources.snapshot().unwrap().is_empty());
        assert_eq!(runtime.stats().nodes, 0);
        assert!(runtime.stats().node_slots <= 3);
    }
    // The same name may be used again once its previous Owner is gone.
    resources
        .bind(&root, "row/0", "v1", || 0, |_| async { Ok::<_, String>(1) })
        .unwrap();
    assert_eq!(resources.len(), 1);
}

#[test]
fn parser_limits_duplicates_versions_unknown_fields_and_names_are_explicit_errors() {
    let mut snapshot = Snapshot::new(Limits {
        bytes: 1024,
        records: 1,
    });
    snapshot.seed("a", "v1", &1, Ok::<_, &String>(&2)).unwrap();
    assert!(matches!(
        snapshot.seed("a", "v1", &1, Ok::<_, &String>(&3)),
        Err(Error::Duplicate(_))
    ));
    assert!(matches!(
        snapshot.seed("b", "v1", &1, Ok::<_, &String>(&3)),
        Err(Error::Limit(_))
    ));
    let json = snapshot.to_json().unwrap();
    assert!(matches!(
        Snapshot::from_json(
            &json,
            Limits {
                bytes: 1,
                records: 1
            }
        ),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        Snapshot::from_json(
            &json,
            Limits {
                bytes: 1024,
                records: 0
            }
        ),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        Snapshot::from_json(
            &json.replace("\"version\":1", "\"version\":99"),
            Limits::default()
        ),
        Err(Error::Version(99))
    ));
    assert!(matches!(
        Snapshot::from_json(
            &json.replace("\"schema\"", "\"extra\":true,\"schema\""),
            Limits::default()
        ),
        Err(Error::Json(_))
    ));
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let record = &value["records"][0];
    let duplicate = format!("{{\"version\":1,\"records\":[{record},{record}]}}");
    assert!(matches!(
        Snapshot::from_json(&duplicate, Limits::default()),
        Err(Error::Duplicate(_))
    ));
    let mut tiny = Snapshot::new(Limits {
        bytes: 80,
        records: 8,
    });
    assert!(matches!(
        tiny.seed("a", "v1", &1, Ok::<_, &String>(&"x".repeat(100))),
        Err(Error::Limit(_))
    ));
    assert!(tiny.is_empty());
    assert!(matches!(
        tiny.seed("", "v1", &1, Ok::<_, &String>(&2)),
        Err(Error::InvalidName)
    ));
    assert!(matches!(
        tiny.seed("a", "", &1, Ok::<_, &String>(&2)),
        Err(Error::InvalidSchema)
    ));
}

#[test]
fn unused_seeds_are_explicitly_discarded_and_snapshot_order_is_stable() {
    let mut snapshot = Snapshot::default();
    for name in ["z", "a", "unmounted"] {
        snapshot
            .seed(name, "v1", &0, Ok::<_, &String>(&name.to_owned()))
            .unwrap();
    }
    let resources = Resources::restore(snapshot).unwrap();
    let runtime = Runtime::new();
    let owner = runtime.owner();
    for name in ["z", "a"] {
        resources
            .bind(
                &owner,
                name,
                "v1",
                || 0,
                |_| async { Ok::<_, String>(String::new()) },
            )
            .unwrap();
    }
    runtime.flush().unwrap();
    let json = resources.snapshot().unwrap().to_json().unwrap();
    assert!(json.find("\"name\":\"a\"").unwrap() < json.find("\"name\":\"z\"").unwrap());
    assert!(!json.contains("unmounted"));
    assert_eq!(resources.remaining(), 1);
    assert_eq!(resources.discard_unused(), ["unmounted"]);
    assert_eq!(resources.remaining(), 0);
}
