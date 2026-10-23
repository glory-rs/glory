use glory_ui::list::{Heights, Layout};

#[test]
fn sparse_measurements_match_dense_prefix_windows_and_explicit_splices() {
    let mut dense = Layout::new(Heights::variable(1000, 24.0).unwrap(), 4);
    let mut sparse = Layout::new(Heights::sparse(1000, 24.0).unwrap(), 4);
    let mut random = 123456_u64;
    for tick in 0..6000 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let index = (random >> 20) as usize % 1000;
        let height = if tick % 3 == 0 {
            24.0
        } else {
            1.0 + ((random >> 32) % 600) as f64 / 8.0
        };
        dense.heights_mut().measure(index, height).unwrap();
        sparse.heights_mut().measure(index, height).unwrap();
        assert_eq!(
            dense.heights().prefix(index).unwrap(),
            sparse.heights().prefix(index).unwrap()
        );
        assert_eq!(dense.heights().total(), sparse.heights().total());
        let offset = ((random >> 16) % 30_000) as f64 + 0.125;
        assert_eq!(
            dense.window(offset, 600.0).unwrap(),
            sparse.window(offset, 600.0).unwrap()
        );
    }
    dense.splice(100..260, 36, 24.0, None).unwrap();
    sparse.splice(100..260, 36, 24.0, None).unwrap();
    assert_eq!(dense.heights().len(), sparse.heights().len());
    for index in 0..=dense.heights().len() {
        assert_eq!(
            dense.heights().prefix(index).unwrap(),
            sparse.heights().prefix(index).unwrap()
        );
    }
}

#[test]
fn billion_row_estimates_allocate_nothing_and_fractional_eviction_leaves_no_history() {
    let mut heights = Heights::sparse(1_000_000_000, 24.0).unwrap();
    assert_eq!(heights.storage_len(), 0);
    assert_eq!(heights.total(), 24_000_000_000.0);
    let mut previous = None;
    for visit in 0..20_000 {
        let index = visit * 7919 % heights.len();
        if let Some(previous) = previous {
            heights.measure(previous, 24.0).unwrap();
        }
        heights.measure(index, 31.37).unwrap();
        assert!(heights.storage_len() <= 32);
        assert!(heights.storage_capacity() <= 64);
        assert!((heights.total() - 24_000_000_007.37).abs() < 0.00001);
        previous = Some(index);
    }
    heights.measure(previous.unwrap(), 24.0).unwrap();
    assert_eq!(heights.storage_len(), 0);
    assert_eq!(heights.total(), 24_000_000_000.0);
}
