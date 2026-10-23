use glory::prelude::*;

struct Counter;
impl Widget for Counter {
    fn build(&mut self, scope: &Scope) -> Result {
        let count = scope.cage(0_u64);
        col()
            .class("counter")
            .fill((
                text().content("Counter").kind(TextKind::Heading(1)),
                text().content(count.map(|count| count.to_string())),
                button()
                    .label("Add")
                    .on(activate, move |_| count.revise(|value| *value += 1)),
            ))
            .show_in(scope)
    }
}

fn main() -> Result {
    let app = App::memory();
    app.mount(Counter)?;
    println!("semantic nodes: {}", app.stats().nodes);
    // Memory is a reference interpreter; Web/Blitz hosts will own actual pixels.
    Ok(())
}
