//! Mapper-only Role workload. Setup and result formatting stay outside timing.
use glory_html::{Edit, Mapper, Op, Sink};
use glory_render::*;

pub const CASES: [&str; 8] = [
    "plain",
    "heading",
    "navigation",
    "grid",
    "relations",
    "dialog",
    "same_role",
    "named_reset",
];

pub fn descriptor(case: usize) -> (Descriptor, [Option<Role>; 2]) {
    let mut node = Descriptor::new(Props::View);
    let mut metadata = Semantics::default();
    let roles = match case {
        0 => [Some(Role::Group), Some(Role::Status)],
        1 => [Some(Role::Heading(2)), Some(Role::Heading(3))],
        2 => {
            metadata.navigation = Some(Orientation::Horizontal);
            [Some(Role::Group), Some(Role::Toolbar)]
        }
        3 => [Some(Role::Group), Some(Role::Grid)],
        4 | 6 | 7 => {
            node.name = Some("A name retained across role updates".repeat(16));
            metadata.labelled_by =
                Some(SemanticId::new("label-".to_owned() + &"x".repeat(240)).unwrap());
            metadata.described_by =
                Some(SemanticId::new("description-".to_owned() + &"y".repeat(240)).unwrap());
            metadata.id = Some(SemanticId::new("region-".to_owned() + &"z".repeat(240)).unwrap());
            metadata.controls =
                Some(SemanticId::new("target-".to_owned() + &"w".repeat(240)).unwrap());
            metadata.busy = Some(true);
            metadata.environment = Some(Environment {
                direction: Direction::Rtl,
                language: LanguageTag::new("ar").unwrap(),
            });
            match case {
                6 => [Some(Role::Group), Some(Role::Group)],
                7 => [Some(Role::Group), None],
                _ => [Some(Role::Group), Some(Role::Status)],
            }
        }
        5 => {
            node.name = Some("Decision".into());
            node.focus = FocusMode::Programmatic;
            metadata.id = Some(SemanticId::new("dialog").unwrap());
            metadata.layer = Some(Layer {
                initial_focus: Some(SemanticId::new("cancel").unwrap()),
                ..Default::default()
            });
            [Some(Role::Dialog), Some(Role::AlertDialog)]
        }
        _ => panic!("unknown role workload"),
    };
    node.role = roles[0];
    if case >= 2 {
        node.semantics = Some(Box::new(metadata));
    }
    node.validate().unwrap();
    node.patched(Patch::Role(Change::Set(roles[1]))).unwrap();
    (node, roles)
}

#[derive(Default)]
struct Counter {
    patches: u64,
    edits: u64,
    checksum: u64,
}
impl Sink for Counter {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        if let Op::Patch { edits, .. } = op {
            self.patches += 1;
            self.edits += edits.len() as u64;
            for edit in edits {
                let Edit::Attribute { name, value, .. } = edit else {
                    panic!("Role changed a property")
                };
                for byte in name
                    .bytes()
                    .chain(value.as_deref().unwrap_or("<removed>").bytes())
                {
                    self.checksum = self.checksum.wrapping_mul(16777619) ^ u64::from(byte);
                }
            }
        }
        Ok(())
    }
}

pub struct Workload {
    mapper: Mapper,
    node: NodeId,
    roles: [Option<Role>; 2],
    sink: Counter,
}
impl Workload {
    pub fn new(case: usize) -> Self {
        let (node, roles) = descriptor(case);
        let runtime = RuntimeId::new();
        let id = NodeIds::new(runtime).allocate();
        let mut mapper = Mapper::new(runtime);
        let mut sink = Counter::default();
        mapper
            .apply(
                &[
                    Mutation::Create {
                        id,
                        node: Box::new(node),
                    },
                    Mutation::Place {
                        id,
                        parent: None,
                        before: None,
                    },
                ],
                &mut sink,
            )
            .unwrap();
        Self {
            mapper,
            node: id,
            roles,
            sink,
        }
    }
    pub fn run(&mut self, pairs: usize) {
        assert!(pairs <= 100_000);
        self.sink = Counter::default();
        for _ in 0..pairs {
            for role in [self.roles[1], self.roles[0]] {
                std::hint::black_box(
                    self.mapper
                        .apply(
                            &[Mutation::Patch {
                                id: self.node,
                                patch: Patch::Role(Change::Set(role)),
                            }],
                            &mut self.sink,
                        )
                        .unwrap(),
                );
            }
        }
        assert_eq!(self.sink.patches, (pairs * 2) as u64);
    }
    pub fn result(&self) -> String {
        format!(
            "{{\"patches\":{},\"edits\":{},\"checksum\":\"{}\"}}",
            self.sink.patches, self.sink.edits, self.sink.checksum
        )
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    thread_local! { static ACTIVE: RefCell<Option<Workload>> = const { RefCell::new(None) }; }
    #[wasm_bindgen]
    pub fn setup(case: usize) {
        ACTIVE.with(|slot| *slot.borrow_mut() = Some(Workload::new(case)));
    }
    #[wasm_bindgen]
    pub fn run(pairs: usize) {
        ACTIVE.with(|slot| slot.borrow_mut().as_mut().unwrap().run(pairs));
    }
    #[wasm_bindgen]
    pub fn result() -> String {
        ACTIVE.with(|slot| slot.borrow().as_ref().unwrap().result())
    }
    #[wasm_bindgen]
    pub fn clear() {
        ACTIVE.with(|slot| drop(slot.borrow_mut().take()));
    }
}
