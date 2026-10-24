use glory_core::Runtime;
#[test]
fn live_shape_is_aggregate_only_and_retirement_removes_its_counts() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(1);
    let mapped = source.map(|value| value + 1);
    owner.binding(move || {
        std::hint::black_box(mapped.get());
    });
    runtime.flush().unwrap();
    let shape = runtime.graph_stats();
    assert_eq!(shape.nodes, [1, 1, 0, 1, 0]);
    assert_eq!(shape.dependencies[0][0], 1);
    assert_eq!(shape.dependencies[1][1], 1);
    assert_eq!(shape.dependencies[3][1], 1);
    assert_eq!(shape.subscribers[0][1], 1);
    assert_eq!(shape.subscribers[1][1], 1);
    source.revise(|value| *value += 1);
    runtime.flush().unwrap();
    assert_eq!(runtime.graph_stats(), shape);
    owner.dispose();
    assert_eq!(runtime.graph_stats(), Default::default());
}
