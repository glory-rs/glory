#[path = "../mixed.rs"]
mod mixed;
fn main() {
    let snapshot = g::ssr::snapshot_with(|scope| mixed::view(&mixed::Model::new(scope))).unwrap();
    println!("{}", snapshot.html);
}
