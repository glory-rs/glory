use glory::{Runtime, core::AccessError};

fn main() -> Result<(), AccessError> {
    let runtime = Runtime::new();
    let scope = runtime.scope();
    let count = scope.cage(1_u64);
    let parity = scope.bond(move || count.get() % 2).with_partial_eq();
    scope.binding(move || println!("parity: {}", parity.get()));
    runtime.flush()?;
    runtime.batch(|| {
        count.revise(|value| *value = 2);
        count.revise(|value| *value = 3);
    })?;
    Ok(())
}
