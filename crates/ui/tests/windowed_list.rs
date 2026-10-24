use glory_render::memory::Memory;
use glory_render::{
    Change, Descriptor, Mutation, NodeId, NodeIds, Patch, Props, Render, RuntimeId, TextProps,
};
use glory_ui::list::{Anchor, Cache, Error, Heights, Layout, ReconcileStats, RowHost, ScrollSpace};

struct Host {
    ids: NodeIds,
    memory: Memory,
    root: NodeId,
    keys_read: usize,
    // Only tests that reorder data need a backing vector. Large data is lazy.
    data: Option<Vec<u64>>,
}

impl Host {
    fn new() -> Self {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let root = ids.allocate();
        let mut memory = Memory::new(runtime);
        memory
            .apply(&[
                Mutation::Create {
                    id: root,
                    node: Box::new(Descriptor::new(Props::View)),
                },
                Mutation::Place {
                    id: root,
                    parent: None,
                    before: None,
                },
            ])
            .unwrap();
        Self {
            ids,
            memory,
            root,
            keys_read: 0,
            data: None,
        }
    }
    fn rendered_keys(&self) -> Vec<u64> {
        self.memory
            .node(self.root)
            .unwrap()
            .children
            .iter()
            .map(|id| {
                let Props::Text(p) = &self.memory.node(*id).unwrap().descriptor.view().props else {
                    panic!()
                };
                p.content.split(':').next().unwrap().parse().unwrap()
            })
            .collect()
    }
}

impl RowHost for Host {
    type Key = u64;
    type Row = NodeId;
    fn key(&mut self, index: usize) -> u64 {
        self.keys_read += 1;
        self.data
            .as_ref()
            .map(|data| data[index])
            .unwrap_or(index as u64)
    }
    fn mount(&mut self, index: usize, key: &u64) -> NodeId {
        let id = self.ids.allocate();
        self.memory
            .apply(&[Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::Text(TextProps {
                    content: format!("{key}:{index}"),
                    ..TextProps::default()
                }))),
            }])
            .unwrap();
        id
    }
    fn update(&mut self, index: usize, key: &u64, row: &mut NodeId) {
        self.memory
            .apply(&[Mutation::Patch {
                id: *row,
                patch: Patch::Text(Change::Set(format!("{key}:{index}"))),
            }])
            .unwrap();
    }
    fn place(&mut self, row: &NodeId, before: Option<&NodeId>) {
        self.memory
            .apply(&[Mutation::Place {
                id: *row,
                parent: Some(self.root),
                before: before.copied(),
            }])
            .unwrap();
    }
    fn dispose(&mut self, row: NodeId) {
        self.memory.apply(&[Mutation::Dispose { id: row }]).unwrap();
        assert!(self.ids.release(row));
    }
}

#[test]
fn million_rows_mount_only_the_window_and_recycle_every_resource() {
    let layout = Layout::new(Heights::fixed(1_000_000, 24.0).unwrap(), 4);
    assert_eq!(layout.heights().storage_len(), 0);
    let mut cache = Cache::default();
    let mut host = Host::new();
    let window = layout.window(12_000_000.0, 600.0).unwrap();
    assert_eq!(window.visible, 500_000..500_025);
    assert_eq!(window.render, 499_996..500_029);
    let stats = cache
        .reconcile(window.render.clone(), 1, &mut host)
        .unwrap();
    assert_eq!(stats.created, 33);
    assert_eq!(stats.keys_read, 33);
    assert_eq!(host.memory.len(), 34);
    let retained = *cache.row(&500_000).unwrap();
    let next = layout.window(12_000_024.0, 600.0).unwrap();
    let stats = cache.reconcile(next.render.clone(), 1, &mut host).unwrap();
    assert_eq!(
        (stats.created, stats.disposed, stats.updated, stats.moved),
        (1, 1, 0, 0)
    );
    assert_eq!(cache.row(&500_000), Some(&retained));
    // A pixel change inside the same window does not even read the data source.
    assert_eq!(
        cache.reconcile(next.render, 1, &mut host).unwrap(),
        ReconcileStats::default()
    );
    for index in (0..1_000_000).step_by(1003) {
        let window = layout.window(index as f64 * 24.0, 600.0).unwrap();
        let stats = cache.reconcile(window.render, 1, &mut host).unwrap();
        assert!(stats.keys_read <= 33);
        assert!(cache.len() <= 33);
        assert_eq!(host.memory.len(), cache.len() + 1);
        assert!(host.ids.capacity() <= 34);
    }
    cache.clear(&mut host);
    assert!(cache.is_empty());
    assert_eq!(host.memory.len(), 1);
    assert_eq!(host.ids.len(), 1);
    assert!(host.memory.retained_slots() <= 34);
}

#[test]
fn reorder_preserves_row_identity_and_uses_minimal_moves() {
    let mut host = Host::new();
    host.data = Some(vec![10, 20, 30, 40]);
    let mut cache = Cache::default();
    cache.reconcile(0..4, 1, &mut host).unwrap();
    let before: Vec<_> = [10, 20, 30, 40].map(|key| *cache.row(&key).unwrap()).into();
    host.data = Some(vec![20, 30, 40, 10]);
    let stats = cache.reconcile(0..4, 2, &mut host).unwrap();
    assert_eq!(
        (stats.created, stats.disposed, stats.moved, stats.updated),
        (0, 0, 1, 4)
    );
    assert_eq!(host.rendered_keys(), [20, 30, 40, 10]);
    for (i, key) in [10, 20, 30, 40].into_iter().enumerate() {
        assert_eq!(cache.row(&key), Some(&before[i]));
    }
    host.data = Some(vec![20, 20, 40, 10]);
    assert_eq!(
        cache.reconcile(0..4, 3, &mut host),
        Err(Error::DuplicateKey)
    );
    assert_eq!(host.rendered_keys(), [20, 30, 40, 10]);
}

#[test]
fn mixed_window_edits_match_an_independent_minimum_move_oracle() {
    fn visit(values: &mut [u64], at: usize, f: &mut impl FnMut(&[u64])) {
        if at == values.len() {
            f(values);
        } else {
            for i in at..values.len() {
                values.swap(at, i);
                visit(values, at + 1, f);
                values.swap(at, i);
            }
        }
    }
    let mut host = Host::new();
    let mut cache = Cache::default();
    let mut revision = 0;
    visit(&mut [10, 20, 30, 40, 50, 60], 0, &mut |permutation| {
        revision += 1;
        let old: Vec<_> = cache
            .order()
            .iter()
            .map(|key| (*key, *cache.row(key).unwrap()))
            .collect();
        // Vary window membership as well as order, including empty windows.
        let count = revision as usize % 7;
        host.data = Some(permutation.to_vec());
        let expected = &permutation[..count];
        let positions: Vec<_> = expected
            .iter()
            .filter_map(|key| old.iter().position(|(old, _)| old == key))
            .collect();
        // Quadratic dynamic programming is deliberately independent of the
        // production binary-search LIS and small enough for exhaustive tests.
        let mut lengths = vec![1; positions.len()];
        for i in 0..positions.len() {
            for j in 0..i {
                if positions[j] < positions[i] {
                    lengths[i] = lengths[i].max(lengths[j] + 1);
                }
            }
        }
        let stable = lengths.into_iter().max().unwrap_or(0);
        let stats = cache.reconcile(0..count, revision, &mut host).unwrap();
        assert_eq!(stats.moved, positions.len() - stable);
        assert_eq!(stats.created, count - positions.len());
        assert_eq!(stats.disposed, old.len() - positions.len());
        assert_eq!(stats.updated, positions.len());
        assert_eq!(host.rendered_keys(), expected);
        assert_eq!(cache.order(), expected);
        for (key, identity) in old {
            if expected.contains(&key) {
                assert_eq!(cache.row(&key), Some(&identity));
            }
        }
        // A rejected edit must not corrupt committed row positions used by
        // the next permutation or the no-op shortcut.
        if count >= 2 {
            host.data = Some(vec![expected[0]; count]);
            let before = host.memory.stats();
            assert_eq!(
                cache.reconcile(0..count, revision + 1, &mut host),
                Err(Error::DuplicateKey)
            );
            assert_eq!(host.memory.stats(), before);
            host.data = Some(permutation.to_vec());
            assert_eq!(
                cache.reconcile(0..count, revision, &mut host).unwrap(),
                ReconcileStats::default()
            );
        }
    });
    assert_eq!(revision, 720);
    cache.clear(&mut host);
    assert_eq!(host.memory.len(), 1);
}

#[test]
fn distant_scroll_does_not_clone_departing_keys() {
    use std::cell::Cell;
    use std::hash::{Hash, Hasher};
    use std::rc::Rc;

    struct Key {
        id: usize,
        clones: Rc<Cell<usize>>,
    }
    impl Clone for Key {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                id: self.id,
                clones: self.clones.clone(),
            }
        }
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Key {}
    impl Hash for Key {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.id.hash(state);
        }
    }
    struct Indexed {
        old_clones: Rc<Cell<usize>>,
        current_clones: Rc<Cell<usize>>,
        live: usize,
    }
    impl RowHost for Indexed {
        type Key = Key;
        type Row = usize;
        fn key(&mut self, index: usize) -> Key {
            Key {
                id: index,
                clones: if index < 64 {
                    self.old_clones.clone()
                } else {
                    self.current_clones.clone()
                },
            }
        }
        fn mount(&mut self, index: usize, _: &Key) -> usize {
            self.live += 1;
            index
        }
        fn update(&mut self, _: usize, _: &Key, _: &mut usize) {}
        fn place(&mut self, _: &usize, _: Option<&usize>) {}
        fn dispose(&mut self, _: usize) {
            self.live -= 1;
        }
    }
    let old_clones = Rc::new(Cell::new(0));
    let mut host = Indexed {
        old_clones: old_clones.clone(),
        current_clones: Rc::new(Cell::new(0)),
        live: 0,
    };
    let mut cache = Cache::default();
    cache.reconcile(0..64, 1, &mut host).unwrap();
    old_clones.set(0);
    let stats = cache
        .reconcile(99_999_936..100_000_000, 1, &mut host)
        .unwrap();
    assert_eq!(
        (stats.created, stats.disposed, stats.moved, stats.keys_read),
        (64, 64, 0, 64)
    );
    assert_eq!(host.live, 64);
    assert_eq!(old_clones.get(), 0);
    cache.clear(&mut host);
    assert_eq!(host.live, 0);
    assert_eq!(old_clones.get(), 0);
}

#[test]
fn variable_heights_match_a_naive_reference_after_random_measurements() {
    let mut layout = Layout::new(Heights::variable(1000, 20.0).unwrap(), 3);
    let mut values = vec![20.0; 1000];
    let mut seed = 19_u64;
    for _ in 0..2000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let index = (seed >> 32) as usize % values.len();
        let height = (seed % 97 + 1) as f64;
        values[index] = height;
        layout.measure(index, height, None).unwrap();
        let total: f64 = values.iter().sum();
        assert_eq!(layout.heights().total(), total);
        let offset = (seed % total as u64) as f64;
        let window = layout.window(offset, 300.0).unwrap();
        let mut prefix = 0.0;
        let expected: Vec<_> = values
            .iter()
            .enumerate()
            .filter_map(|(i, height)| {
                let start = prefix;
                prefix += height;
                (prefix > window.offset && start < window.offset + 300.0).then_some(i)
            })
            .collect();
        assert_eq!(
            window.visible,
            *expected.first().unwrap()..expected.last().unwrap() + 1
        );
        assert_eq!(
            window.top,
            values[..window.render.start].iter().sum::<f64>()
        );
        assert_eq!(
            window.bottom,
            values[window.render.end..].iter().sum::<f64>()
        );
    }
}

#[test]
fn measurements_and_prepend_keep_the_logical_anchor() {
    let mut layout = Layout::new(Heights::variable(100, 20.0).unwrap(), 2);
    let anchor = layout.anchor(405.0).unwrap().unwrap();
    assert_eq!(
        anchor,
        Anchor {
            index: 20,
            within: 5.0
        }
    );
    let (_, offset) = layout.measure(0, 70.0, Some(anchor)).unwrap();
    assert_eq!(offset, Some(455.0));
    assert_eq!(
        layout.splice(0..0, 10, 20.0, Some(anchor)).unwrap(),
        Some(655.0)
    );
    assert_eq!(layout.heights().height(10), Some(70.0));
    layout.heights_mut().splice(10..11, 0, 20.0).unwrap();
    assert_eq!(layout.heights().len(), 109);
    assert_eq!(layout.heights().total(), 2180.0);
}

#[test]
fn removing_the_anchor_selects_the_next_survivor_or_the_tail() {
    let mut layout = Layout::new(Heights::fixed(100, 20.0).unwrap(), 2);
    assert_eq!(
        layout
            .splice(
                19..22,
                0,
                20.0,
                Some(Anchor {
                    index: 20,
                    within: 5.0
                })
            )
            .unwrap(),
        Some(380.0)
    );
    assert_eq!(
        layout
            .splice(
                90..97,
                0,
                20.0,
                Some(Anchor {
                    index: 95,
                    within: 5.0
                })
            )
            .unwrap(),
        Some(1780.0)
    );
    assert_eq!(
        layout
            .splice(
                0..90,
                0,
                20.0,
                Some(Anchor {
                    index: 89,
                    within: 5.0
                })
            )
            .unwrap(),
        None
    );
}

#[test]
fn random_data_edits_match_rendered_order_without_remounting_retained_keys() {
    use std::collections::HashMap;
    let mut host = Host::new();
    host.data = Some((0..20).collect());
    let mut cache = Cache::default();
    cache.reconcile(0..20, 0, &mut host).unwrap();
    let mut seed = 7_u64;
    for revision in 1..=500 {
        let previous: HashMap<_, _> = cache
            .order()
            .iter()
            .map(|key| (*key, *cache.row(key).unwrap()))
            .collect();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let data = host.data.as_mut().unwrap();
        match seed % 4 {
            0 => {
                let at = seed as usize % (data.len() + 1);
                data.insert(at, revision + 1000);
            }
            1 if !data.is_empty() => {
                data.remove(seed as usize % data.len());
            }
            2 if !data.is_empty() => {
                let by = seed as usize % data.len();
                data.rotate_left(by);
            }
            _ if !data.is_empty() => {
                let last = data.len() - 1;
                data.swap(0, last);
            }
            _ => {}
        }
        let expected = data.clone();
        cache
            .reconcile(0..expected.len(), revision, &mut host)
            .unwrap();
        assert_eq!(host.rendered_keys(), expected);
        assert_eq!(host.memory.len(), expected.len() + 1);
        for key in cache.order() {
            if let Some(previous) = previous.get(key) {
                assert_eq!(cache.row(key), Some(previous));
            }
        }
    }
}

#[test]
fn bounded_scroll_coordinates_reach_the_end_and_preserve_wheel_deltas() {
    let total = 24_000_000_000.0;
    let mut space = ScrollSpace::new(total, 600.0, 1_000_000.0).unwrap();
    let first = space.seek(12_000_000_005.0).unwrap();
    assert!(first.rebased);
    assert_eq!(space.project(first.logical), first.physical);
    assert!(first.physical < space.extent());
    let next = space.scrolled(first.physical + 1.0).unwrap();
    assert_eq!(next.logical, first.logical + 1.0);
    let end = space.seek(total).unwrap();
    assert_eq!(end.logical, total - 600.0);
    assert_eq!(end.physical, space.extent() - 600.0);
    let at_end = space.scrolled(end.physical).unwrap();
    assert_eq!(at_end.logical, end.logical);
    let home = space.seek(0.0).unwrap();
    assert_eq!(
        (home.logical, home.physical, space.origin()),
        (0.0, 0.0, 0.0)
    );
    let middle = space.scrolled(900_000.0).unwrap();
    assert!(middle.rebased);
    assert_eq!(middle.logical, 900_000.0);
    assert_eq!(space.project(middle.logical), middle.physical);
    let resized = space.resize(100.0, 600.0, middle.logical).unwrap();
    assert_eq!(
        (resized.logical, resized.physical, space.extent()),
        (0.0, 0.0, 100.0)
    );
}

#[test]
fn empty_exact_boundaries_and_invalid_numbers_are_explicit() {
    let layout = Layout::new(Heights::fixed(100, 20.0).unwrap(), 0);
    assert_eq!(layout.window(20.0, 20.0).unwrap().render, 1..2);
    assert_eq!(layout.window(20.5, 20.0).unwrap().render, 1..3);
    assert_eq!(layout.window(9000.0, 20.0).unwrap().render, 99..100);
    assert!(layout.window(f64::NAN, 20.0).is_err());
    assert!(layout.window(0.0, -1.0).is_err());
    assert_eq!(layout.window(0.0, 0.0).unwrap().render, 0..0);
    let empty = Layout::new(Heights::variable(0, 20.0).unwrap(), 10);
    assert_eq!(empty.window(0.0, 20.0).unwrap().render, 0..0);
    assert_eq!(empty.anchor(0.0).unwrap(), None);
    assert!(Heights::fixed(100, 0.0).is_err());
    assert!(Heights::variable(1, f64::INFINITY).is_err());
}

#[test]
fn a_single_pin_joins_the_sorted_window_without_growing_a_visited_cache() {
    let mut host = Host::new();
    let mut cache = Cache::default();
    cache.reconcile(0..20, 1, &mut host).unwrap();
    let focused = *cache.row(&3).unwrap();
    for start in (100..1_000_000).step_by(7919) {
        let stats = cache
            .reconcile_pinned(start..start + 20, 1, Some((3, 3)), &mut host)
            .unwrap();
        assert_eq!(stats.keys_read, 20);
        assert_eq!(cache.len(), 21);
        assert_eq!(cache.row(&3), Some(&focused));
        assert_eq!(cache.order()[0], 3);
        assert_eq!(host.rendered_keys(), cache.order());
        assert!(host.ids.capacity() <= 22);
    }
    cache
        .reconcile_pinned(0..20, 1, Some((3, 3)), &mut host)
        .unwrap();
    assert_eq!(cache.len(), 20);
    assert_eq!(cache.row(&3), Some(&focused));
    assert_eq!(host.rendered_keys(), (0..20).collect::<Vec<_>>());
    // Pin changes invalidate an otherwise identical window; invalid pin keys
    // must be diagnosed before deleting or moving anything.
    let before = host.memory.stats();
    assert_eq!(
        cache.reconcile_pinned(0..20, 1, Some((50, 3)), &mut host),
        Err(Error::DuplicateKey)
    );
    assert_eq!(
        cache.reconcile_pinned(0..20, 1, Some((3, 9)), &mut host),
        Err(Error::InvalidLookup)
    );
    assert_eq!(host.memory.stats(), before);
    cache
        .reconcile_pinned(100..120, 1, Some((3, 3)), &mut host)
        .unwrap();
    cache.reconcile(100..120, 1, &mut host).unwrap();
    assert!(cache.row(&3).is_none());
    cache.clear(&mut host);
    assert!(cache.is_empty());
}

#[test]
fn impossible_cache_allocations_fail_before_reading_keys_or_changing_rows() {
    let mut host = Host::new();
    let mut cache = Cache::default();
    cache.reconcile(0..4, 0, &mut host).unwrap();
    let reads = host.keys_read;
    let before = host.memory.stats();
    assert_eq!(
        cache.reconcile(0..usize::MAX, 1, &mut host),
        Err(Error::Allocation)
    );
    assert_eq!(host.keys_read, reads);
    assert_eq!(host.memory.stats(), before);
    assert_eq!(cache.len(), 4);
    assert_eq!(
        cache.reconcile_pinned(0..usize::MAX, 1, Some((usize::MAX, u64::MAX)), &mut host),
        Err(Error::Allocation)
    );
}
