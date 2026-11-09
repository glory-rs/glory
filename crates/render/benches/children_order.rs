//! Isolated semantic-order benchmark. Setup and full order/adjacency validation
//! are outside the timer; this does not measure DOM/layout or presentation.
use glory_render::{memory::Memory, *};
use std::{hint::black_box, time::Instant};

fn run(count: usize, scenario: &str) -> u128 {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let parent = ids.allocate();
    tree.apply(&[Mutation::Create {
        id: parent,
        node: Box::new(Descriptor::new(Props::View)),
    }])
    .unwrap();
    let original: Vec<_> = (0..count).map(|_| ids.allocate()).collect();
    for &id in &original {
        tree.apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::View)),
            },
            Mutation::Place {
                id,
                parent: Some(parent),
                before: None,
            },
        ])
        .unwrap();
    }
    let mut expected = original.clone();
    let mut commands = Vec::with_capacity(count);
    match scenario {
        "append" | "prepend" => {
            let incoming: Vec<_> = (0..count).map(|_| ids.allocate()).collect();
            for &id in &incoming {
                tree.apply(&[Mutation::Create {
                    id,
                    node: Box::new(Descriptor::new(Props::View)),
                }])
                .unwrap();
                commands.push(Mutation::Place {
                    id,
                    parent: Some(parent),
                    before: (scenario == "prepend").then_some(original[0]),
                });
            }
            if scenario == "prepend" {
                expected.splice(..0, incoming);
            } else {
                expected.extend(incoming);
            }
        }
        "reverse" | "shuffle" => {
            if scenario == "reverse" {
                expected.reverse();
            } else {
                let mut seed = 17_u64;
                for index in (1..count).rev() {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    expected.swap(index, seed as usize % (index + 1));
                }
            }
            let mut before = None;
            for &id in expected.iter().rev() {
                commands.push(Mutation::Place {
                    id,
                    parent: Some(parent),
                    before,
                });
                before = Some(id);
            }
        }
        "head-retire" => {
            commands.extend(original.iter().map(|&id| Mutation::Dispose { id }));
            expected.clear();
        }
        _ => panic!("unknown scenario"),
    }
    let start = Instant::now();
    let receipt = tree.apply(black_box(&commands)).unwrap();
    let elapsed = start.elapsed().as_nanos();
    black_box(receipt);
    assert_eq!(tree.node(parent).unwrap().children.as_ref(), expected);
    for (index, &id) in expected.iter().enumerate() {
        assert_eq!(
            tree.next_sibling(id).unwrap(),
            expected.get(index + 1).copied()
        );
    }
    elapsed
}

fn main() {
    let repeats = std::env::args()
        .nth(1)
        .map_or(5, |value| value.parse::<usize>().unwrap());
    println!("count,scenario,repeat,nanoseconds");
    for count in [1_000, 10_000, 50_000] {
        for scenario in ["append", "prepend", "reverse", "shuffle", "head-retire"] {
            for repeat in 0..repeats {
                println!("{count},{scenario},{repeat},{}", run(count, scenario));
            }
        }
    }
}
