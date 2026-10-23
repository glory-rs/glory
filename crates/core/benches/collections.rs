//! CPU/work baseline for explicit structural edits vs conservative revise.
//! Source construction is outside timing; both paths perform identical data
//! operations. Run with CARGO_TARGET_DIR on D:, not the retired scheduler bench.
use glory_core::Runtime;
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
    hint::black_box,
    rc::Rc,
    time::Instant,
};

struct Key(usize, Rc<Cell<usize>>);
impl Hash for Key {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.1.set(self.1.get() + 1);
        self.0.hash(h);
    }
}
impl PartialEq for Key {
    fn eq(&self, o: &Self) -> bool {
        self.1.set(self.1.get() + 1);
        self.0 == o.0
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.1.set(self.1.get() + 1);
        self.0.cmp(&o.0)
    }
}
struct Value(usize); // Deliberately not Clone.
const BATCHES: usize = 20;

macro_rules! bench_map {
    ($run:ident,$map:ty,$kind:literal) => {
        #[allow(clippy::mutable_key_type, reason="Only immutable index participates in key identity; Cell counts lookup work")]
        fn $run(size:usize,visible:usize,op:usize,burst:usize,narrow:bool,repetition:usize) {
            let runtime=Runtime::new();let owner=runtime.owner();let probes=Rc::new(Cell::new(0));
            let data:$map=(0..size).map(|i|(Key(i,probes.clone()),Value(0))).collect();let rows=owner.cage(data);
            let outputs=Rc::new(Cell::new(0));let out=outputs.clone();
            owner.binding(move||{black_box(rows.with(|m|m.len()));out.set(out.get()+1);});
            for i in 0..visible {
                let item=rows.entry(Key(i,probes.clone()));
                let value=owner.bond(move||item.try_with(|v|v.0)).with_partial_eq();let out=outputs.clone();
                owner.binding(move||{black_box(value.get().expect("row exists at batch boundary"));out.set(out.get()+1);});
            }
            runtime.flush().unwrap();let before=runtime.stats();let calls=outputs.get();probes.set(0);
            let index=if op==2 {size+1} else {0};let key=Key(index,probes.clone());let start=Instant::now();
            for batch in 0..BATCHES {
                runtime.batch(||for tick in 0..burst {
                    let value=Value(1+batch*burst+tick);
                    if narrow {
                        if op==1 {black_box(rows.remove(&key));}
                        black_box(rows.insert(Key(index,probes.clone()),value));
                        if op==2 {black_box(rows.remove(&key));}
                    } else {
                        rows.revise(|m| {
                            if op==1 {black_box(m.remove(&key));}
                            black_box(m.insert(Key(index,probes.clone()),value));
                            if op==2 {black_box(m.remove(&key));}
                        });
                    }
                }).unwrap();
            }
            let elapsed=start.elapsed().as_nanos();let key_ops=probes.get();let after=runtime.stats();
            assert_eq!(rows.with(|m|m.len()),size);
            if op!=2 {assert_eq!(rows.with(|m|m.get(&key).unwrap().0),BATCHES*burst);}
            assert_eq!(outputs.get()-calls,BATCHES*if op==2 {1} else {2});
            let expected=if narrow {if op==2 {0}else{BATCHES}} else {visible*BATCHES};
            assert_eq!(after.derived_runs-before.derived_runs,expected as u64);
            assert_eq!((after.fields,after.entry_keys),(visible,visible));
            println!("{{\"kind\":{},\"source\":{},\"visible\":{},\"op\":{},\"burst\":{},\"narrow\":{},\"repetition\":{},\"elapsed_ns\":{},\"key_ops\":{},\"writes\":{},\"marked\":{},\"derived\":{},\"bindings\":{},\"target_keys\":{}}}",
                $kind,size,visible,op,burst,narrow,repetition,elapsed,key_ops,after.source_writes-before.source_writes,after.marked-before.marked,after.derived_runs-before.derived_runs,after.binding_runs-before.binding_runs,after.field_target_keys);
        }
    }
}
bench_map!(btree,BTreeMap<Key,Value>,"\"btree\"");
bench_map!(hash,HashMap<Key,Value>,"\"hash\"");

fn vector(size: usize, visible: usize, op: usize, burst: usize, narrow: bool, repetition: usize) {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage((0..size).collect::<Vec<_>>());
    let calls = Rc::new(Cell::new(0));
    let out = calls.clone();
    owner.binding(move || {
        black_box(rows.with(Vec::len));
        out.set(out.get() + 1);
    });
    for i in 0..visible {
        let item = rows.item(i);
        let value = owner.bond(move || item.get()).with_partial_eq();
        let out = calls.clone();
        owner.binding(move || {
            black_box(value.get());
            out.set(out.get() + 1);
        });
    }
    runtime.flush().unwrap();
    let before = runtime.stats();
    let outputs = calls.get();
    let start = Instant::now();
    for _ in 0..BATCHES {
        runtime
            .batch(|| {
                for _ in 0..burst {
                    if narrow {
                        if op == 0 {
                            rows.push(size);
                            black_box(rows.pop());
                        } else {
                            rows.insert(0, size);
                            black_box(rows.remove(0));
                        }
                    } else {
                        rows.revise(|v| {
                            if op == 0 {
                                v.push(size);
                                black_box(v.pop());
                            } else {
                                v.insert(0, size);
                                black_box(v.remove(0));
                            }
                        });
                    }
                }
            })
            .unwrap();
    }
    let elapsed = start.elapsed().as_nanos();
    let after = runtime.stats();
    assert_eq!(rows.with(Vec::len), size);
    assert_eq!(rows.with(|v| v[0]), 0);
    assert_eq!(calls.get() - outputs, BATCHES);
    assert_eq!(
        after.derived_runs - before.derived_runs,
        if narrow && op == 0 {
            0
        } else {
            (BATCHES * visible) as u64
        }
    );
    println!(
        "{{\"kind\":\"vec\",\"source\":{size},\"visible\":{visible},\"op\":{op},\"burst\":{burst},\"narrow\":{narrow},\"repetition\":{repetition},\"elapsed_ns\":{elapsed},\"writes\":{},\"marked\":{},\"derived\":{},\"bindings\":{},\"target_keys\":{}}}",
        after.source_writes - before.source_writes,
        after.marked - before.marked,
        after.derived_runs - before.derived_runs,
        after.binding_runs - before.binding_runs,
        after.field_target_keys
    );
}
fn main() {
    for size in [1024, 131072] {
        for visible in [32, 256] {
            for burst in [1, 128] {
                for repetition in 0..3 {
                    for narrow in [false, true] {
                        for op in 0..3 {
                            btree(size, visible, op, burst, narrow, repetition);
                            hash(size, visible, op, burst, narrow, repetition);
                        }
                        for op in 0..2 {
                            vector(size, visible, op, burst, narrow, repetition);
                        }
                    }
                }
            }
        }
    }
}
