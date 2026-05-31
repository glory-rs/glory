use glory_hot_reload::{FunctionRegistry, reloadable_fn};

fn main() -> anyhow::Result<()> {
    let registry = FunctionRegistry::new();
    let owner = "counter-root";

    let render_row = reloadable_fn!("counter-row", owner = owner, registry, |value: i32| {
        format!("<button>{value}</button>")
    })?;

    assert_eq!(render_row.call(1), "<button>1</button>");

    registry.replace_with_owner("counter-root", "counter-row", |value: i32| {
        format!("<button class=\"hot\">{value}</button>")
    })?;

    println!("{}", render_row.call(2));
    Ok(())
}
